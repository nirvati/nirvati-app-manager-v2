const crypto = {
    getRandomValues(uint8Array: Uint8Array) {
        const randomHex = _nirvati_getRandomValues(uint8Array.byteLength);
        // Every byte is two hex characters
        for (let i = 0; i < uint8Array.byteLength; i++) {
            uint8Array[i] = parseInt(randomHex.substring(i * 2, i * 2 + 2), 16);
        }
    },
    get subtle() {
        _nirvati_dbg("SubtleCrypto is not (yet) supported in Nirvati's JS engine.");
        return false;
    }
};
