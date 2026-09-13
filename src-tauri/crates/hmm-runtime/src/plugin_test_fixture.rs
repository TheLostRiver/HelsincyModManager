/// Minimal inert PE headers for format checks. Tests never load or execute these bytes.
pub(crate) const X64_DLL: [u8; 512] = {
    let mut bytes = [0; 512];
    bytes[0] = b'M';
    bytes[1] = b'Z';
    bytes[0x3c] = 64;
    bytes[64] = b'P';
    bytes[65] = b'E';
    bytes[68] = 0x64;
    bytes[69] = 0x86;
    bytes[70] = 1;
    bytes[84] = 240;
    bytes[86] = 2;
    bytes[87] = 0x20;
    bytes[88] = 0x0b;
    bytes[89] = 2;
    bytes[148] = 0x80;
    bytes[149] = 1;
    bytes[344] = 16;
    bytes[348] = 0x80;
    bytes[349] = 1;
    bytes
};
