# Nirvati app manager

This is the app system for Nirvati. While it is inspired by and partially based on Citadel's app system, a large part of the codebase has been rewritten.

Apps now consist of two files: A `manifest.yml` file that provides metadata for the app store, and the `app.yml`, which is the actual app definition.
Like with Citadel, app.yml.jinja and manifest.yml.jinja are also supported, which allows you to use Tera (Jinja-like) templates to generate the app.yml and manifest.yml files.

`metadata.yml.jinja` only has access to the `services` variable, which is a list of all apps/services that are installed.

Virtual apps now require their own app dir with a `manifest.yml` file that has the `virtual` key set to `true`.

App config files in Jinja format are also supported.

There is a specific order in which app parts are processed. There may not be any recursive dependencies within a stage:: If an app `A` depends on another app `B`, `B` may not depend on `A`. But if `A`'s app.yml depends on `B`'s app.yml, and `B`'s config files depend on `A`'s app.yml, that is fine.

The processing order is this:

1. manifest.yml.jinja files are processed and all manifest.yml files are collected into registry.json.
1. For any apps which only has installed services as dependencies, the app.yml.jinja files are processed and the app.yml files are converted to docker-compose.yml files.
1. Now, for the same apps, the jinja config files are processed.

### What this does not handle

- Validation of app settings
- Ensuring implementations of "virtual apps" all use the same settings
- Starting/stopping apps
