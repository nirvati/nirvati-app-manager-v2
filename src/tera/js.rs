use anyhow::{anyhow, bail, Result};
use deno_ast::{EmitOptions, ParseParams, SourceTextInfo};
use quick_js::{Context as QuickJSContext, JsValue};
use rand::RngCore;
use serde_json::Value;
use std::{
    collections::HashMap,
    marker::PhantomData,
    path::Path,
    sync::{Arc, Mutex},
};
use tera::{Context, Tera};

pub fn transpile_js_ts_in_thread(path: &Path) -> Result<(String, Vec<String>)> {
    let contents = std::fs::read_to_string(path)?;
    let ext = path
        .extension()
        .ok_or_else(|| anyhow!("Failed to get extension of file"))?
        .to_str()
        .ok_or_else(|| anyhow!("Failed to get extension of file"))?
        .to_string();
    let specifier = format!("file://{}", path.display());
    let transpile_result = std::thread::spawn(move || -> Result<(String, Vec<String>)> {
        // This may execute JS code, so we need to sandbox it
        extrasafe::SafetyContext::new()
            .enable(
                extrasafe::builtins::SystemIO::nothing()
                    .allow_stdout()
                    .allow_stderr(),
            )
            .unwrap()
            .apply_to_current_thread()?;
        let mut exported_funcs = Vec::new();
        let script = deno_ast::parse_script(ParseParams {
            specifier,
            media_type: if ext == "js" {
                deno_ast::MediaType::JavaScript
            } else {
                deno_ast::MediaType::TypeScript
            },
            capture_tokens: false,
            maybe_syntax: None,
            scope_analysis: false,
            text_info: SourceTextInfo::new(contents.into()),
        })?;
        // Get all function names
        for node in &script.script().body {
            if let deno_ast::swc::ast::Stmt::Decl(deno_ast::swc::ast::Decl::Fn(func)) = node {
                if func.function.params.len() == 1
                    && !exported_funcs.contains(&func.ident.sym.to_string())
                {
                    exported_funcs.push(func.ident.sym.to_string());
                }
            }
        }

        let transpiled = script.transpile(&EmitOptions {
            inline_source_map: false,
            inline_sources: false,
            ..EmitOptions::default()
        })?;
        Ok((transpiled.text, exported_funcs))
    });
    let result = transpile_result
        .join()
        .ok()
        .ok_or_else(|| anyhow!("Joining failed"))??;
    Ok(result)
}

pub fn parse_tera_helpers(dir: &Path) -> anyhow::Result<(String, Vec<String>)> {
    let mut code = String::new();
    let mut exported_funcs = Vec::new();
    // Loop through all files in dir that end in .js or .ts.
    // Transpile them to ES2019 using deno_ast
    // Then parse them using quick_js
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_file() {
            let ext = path
                .extension()
                .ok_or_else(|| anyhow!("Failed to get extension of file"))?;
            if ext == "js" || ext == "ts" {
                // I haven't audited the code of the transpiler, so run it in a separate thread without any FS access to prevent it from doing anything malicious
                let (code_additions, exported_func_additions) = transpile_js_ts_in_thread(&path)?;
                code.push_str(&code_additions);
                exported_funcs.extend(exported_func_additions);
            }
        }
    }
    // Put the polyfills at the top of the file
    // They're in OUT_DIR because build.rs transpiles and minifies them for production
    code = format!(
        "{}\n{}\n{}",
        include_str!(concat!(env!("OUT_DIR"), "/polyfills/textencoder.js")),
        include_str!(concat!(env!("OUT_DIR"), "/polyfills/webcrypto.js")),
        code
    );
    Ok((code, exported_funcs))
}

fn js_val_to_serde_val(val: JsValue) -> Result<Value> {
    Ok(match val {
        JsValue::Undefined | JsValue::Null => Value::Null,
        JsValue::Bool(bool) => Value::Bool(bool),
        JsValue::Int(i32) => Value::Number(i32.into()),
        JsValue::Float(f64) => Value::Number(
            serde_json::Number::from_f64(f64)
                .ok_or_else(|| anyhow!("Failed to convert f64 to Number"))?,
        ),
        JsValue::String(string) => Value::String(string),
        JsValue::Array(arr) => Value::Array(
            arr.into_iter()
                .map(js_val_to_serde_val)
                .collect::<Result<Vec<Value>>>()?,
        ),
        JsValue::Object(obj) => Value::Object(serde_json::Map::from_iter(
            obj.into_iter()
                .map(|(key, val)| Ok((key, js_val_to_serde_val(val)?)))
                .collect::<Result<Vec<(String, Value)>>>()?
                .into_iter(),
        )),
        JsValue::Date(date) => Value::String(date.to_rfc3339()),
        JsValue::BigInt(bigint) => Value::String(bigint.to_string()),
        _ => bail!("Failed to convert JsValue to Value"),
    })
}

// This is a hack, but it works, at least for now.
struct CtxWrapper {
    pub ctx: QuickJSContext,
}
unsafe impl Send for CtxWrapper {}
unsafe impl Sync for CtxWrapper {}

pub struct TeraWithJs {
    tera: Tera,
    quickjs_ctx: Arc<Mutex<CtxWrapper>>,
    _not_sync: PhantomData<*mut ()>,
}

impl TeraWithJs {
    pub fn eval(&self, code: &str) -> Result<JsValue> {
        let ctx = self.quickjs_ctx.as_ref().lock();
        let Ok(ctx) = ctx else {
            return Err(anyhow!("Failed to lock context"));
        };
        Ok(ctx.ctx.eval(code)?)
    }

    pub fn render_str(&mut self, input: &str, context: &Context) -> Result<String> {
        Ok(self.tera.render_str(input, context)?)
    }
}

// TODO: Wait for this to be in stable Rust
//impl !Send for TeraWithJs {}
//impl !Sync for TeraWithJs {}

pub fn declare_js_functions(
    mut tera: Tera,
    code: &str,
    exported_funcs: &[String],
) -> Result<TeraWithJs> {
    let ctx = QuickJSContext::new()?;
    ctx.add_callback("_nirvati_getRandomValues", |len: i32| -> JsValue {
        let mut rng = rand::thread_rng();
        let mut bytes = vec![0u8; len as usize];
        rng.fill_bytes(&mut bytes);
        JsValue::String(hex::encode(bytes))
    })?;
    ctx.add_callback("_nirvati_dbg", |msg: String| -> JsValue {
        tracing::debug!("[JS] {}", msg);
        JsValue::Undefined
    })?;
    ctx.eval(code)?;
    let ctx_arc = Arc::new(Mutex::new(CtxWrapper { ctx }));

    for func in exported_funcs {
        let ctx = ctx_arc.clone();
        let fn_name = func.clone();
        tera.register_function(func, move |args: &HashMap<String, Value>| {
            let arg = serde_json::to_string(args)?;
            let ctx = ctx.as_ref().lock();
            let Ok(ctx) = ctx else {
                return Err("Failed to lock context".into());
            };
            let result = ctx.ctx.eval(&format!("{}({})", fn_name, arg));
            if let Ok(result) = result {
                let result = js_val_to_serde_val(result);
                if let Ok(result) = result {
                    Ok(result)
                } else {
                    Err("Failed to convert JS value to serde value".into())
                }
            } else {
                eprintln!("{:#?}", result.err());
                Err(format!("Failed to call JS function {}", fn_name).into())
            }
        });
    }
    Ok(TeraWithJs {
        tera,
        quickjs_ctx: ctx_arc,
        _not_sync: PhantomData,
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::declare_js_functions;
    use quick_js::JsValue;
    use serde_json::Value;
    use tera::Tera;

    #[test]
    fn test_js_execution() {
        let code = r#"
            function math(args) {
                return (args.num1 + 1) * args.num2;
            }"#;
        let mut tera = declare_js_functions(Tera::default(), code, &["math".to_string()]).unwrap();
        let result = tera
            .tera
            .render_str("{{ math(num1=5, num2=2) }}", &tera::Context::new())
            .unwrap();
        assert_eq!(result, "12");
    }

    #[test]
    fn test_async_js_execution() {
        let code = r#"
            async function async_math(args) {
                return new Promise((resolve) => {
                    resolve((args.num1 + 1) * args.num2);
                });
            }"#;
        let mut tera =
            declare_js_functions(Tera::default(), code, &["async_math".to_string()]).unwrap();
        let result = tera
            .tera
            .render_str("{{ async_math(num1=5, num2=2) }}", &tera::Context::new())
            .unwrap();
        assert_eq!(result, "12");
    }

    #[test]
    fn test_js_val_to_serde_val() {
        use super::js_val_to_serde_val;
        let val = JsValue::Object(HashMap::<String, JsValue>::from_iter(vec![
            ("num".to_string(), JsValue::Int(5)),
            ("str".to_string(), JsValue::String("hello".to_string())),
            ("bool".to_string(), JsValue::Bool(true)),
            ("float".to_string(), JsValue::Float(5.5)),
            ("null".to_string(), JsValue::Null),
            ("undefined".to_string(), JsValue::Undefined),
            (
                "array".to_string(),
                JsValue::Array(vec![
                    JsValue::Int(1),
                    JsValue::Int(2),
                    JsValue::Int(3),
                    JsValue::Object(HashMap::<String, JsValue>::from_iter(vec![
                        ("num".to_string(), JsValue::Int(5)),
                        ("str".to_string(), JsValue::String("hello".to_string())),
                        ("bool".to_string(), JsValue::Bool(true)),
                        ("float".to_string(), JsValue::Float(5.5)),
                        ("null".to_string(), JsValue::Null),
                        ("undefined".to_string(), JsValue::Undefined),
                    ])),
                ]),
            ),
            (
                "object".to_string(),
                JsValue::Object(HashMap::<String, JsValue>::from_iter(vec![
                    ("num".to_string(), JsValue::Int(5)),
                    ("str".to_string(), JsValue::String("hello".to_string())),
                    ("bool".to_string(), JsValue::Bool(true)),
                    ("float".to_string(), JsValue::Float(5.5)),
                    ("null".to_string(), JsValue::Null),
                    ("undefined".to_string(), JsValue::Undefined),
                ])),
            ),
        ]));
        let result = js_val_to_serde_val(val).unwrap();
        let expected = Value::Object(
            vec![
                ("num".to_string(), Value::Number(5.into())),
                ("str".to_string(), Value::String("hello".to_string())),
                ("bool".to_string(), Value::Bool(true)),
                (
                    "float".to_string(),
                    Value::Number(serde_json::Number::from_f64(5.5).unwrap()),
                ),
                ("null".to_string(), Value::Null),
                ("undefined".to_string(), Value::Null),
                (
                    "array".to_string(),
                    Value::Array(vec![
                        Value::Number(1.into()),
                        Value::Number(2.into()),
                        Value::Number(3.into()),
                        Value::Object(serde_json::Map::from_iter(vec![
                            ("num".to_string(), Value::Number(5.into())),
                            ("str".to_string(), Value::String("hello".to_string())),
                            ("bool".to_string(), Value::Bool(true)),
                            (
                                "float".to_string(),
                                Value::Number(serde_json::Number::from_f64(5.5).unwrap()),
                            ),
                            ("null".to_string(), Value::Null),
                            ("undefined".to_string(), Value::Null),
                        ])),
                    ]),
                ),
                (
                    "object".to_string(),
                    Value::Object(serde_json::Map::from_iter(vec![
                        ("num".to_string(), Value::Number(5.into())),
                        ("str".to_string(), Value::String("hello".to_string())),
                        ("bool".to_string(), Value::Bool(true)),
                        (
                            "float".to_string(),
                            Value::Number(serde_json::Number::from_f64(5.5).unwrap()),
                        ),
                        ("null".to_string(), Value::Null),
                        ("undefined".to_string(), Value::Null),
                    ])),
                ),
            ]
            .into_iter()
            .collect(),
        );
        assert_eq!(result, expected);
    }
}
