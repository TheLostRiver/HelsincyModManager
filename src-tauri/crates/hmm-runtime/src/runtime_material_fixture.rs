/// 人工最小纹理表，尾部材质数据不参与路径改写。
pub(super) const fn single_texture_material(reference: &str) -> [u8; 376] {
    let mut bytes = [0; 376];
    bytes[0] = b'M';
    bytes[1] = b'R';
    bytes[2] = b'L';
    bytes[4] = 12;
    bytes[16] = 1;
    bytes[20] = 1;
    bytes[24] = 40;
    bytes[32] = 56;
    bytes[33] = 1;
    bytes[40] = 0xeb;
    bytes[41] = 0x5d;
    bytes[42] = 0x1f;
    bytes[43] = 0x24;
    let path = reference.as_bytes();
    assert!(path.len() < 256);
    let mut index = 0;
    while index < path.len() {
        bytes[56 + index] = path[index];
        index += 1;
    }
    bytes
}
