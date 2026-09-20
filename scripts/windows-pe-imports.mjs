// Read-only PE inspection. No binary is loaded or executed.
export function inspectWindowsPe(input) {
  const bytes = Buffer.isBuffer(input) ? input : Buffer.from(input);
  const invalid = () => { throw new Error("Invalid Windows PE binary"); };
  const range = (offset, size) => {
    if (!Number.isSafeInteger(offset) || offset < 0 || size < 0 || offset + size > bytes.length) {
      invalid();
    }
  };
  range(0, 64);
  if (bytes.readUInt16LE(0) !== 0x5a4d) invalid();
  const pe = bytes.readUInt32LE(0x3c);
  if (pe < 64) invalid();
  range(pe, 24);
  if (bytes.readUInt32LE(pe) !== 0x4550) invalid();
  const sectionCount = bytes.readUInt16LE(pe + 6);
  if (sectionCount === 0 || sectionCount > 96) invalid();
  const optional = pe + 24;
  const optionalSize = bytes.readUInt16LE(pe + 20);
  range(optional, optionalSize);
  if (optionalSize < 2) invalid();
  const magic = bytes.readUInt16LE(optional);
  const directoryOffset = magic === 0x20b ? 112 : magic === 0x10b ? 96 : 0;
  if (directoryOffset === 0 || optionalSize < directoryOffset) invalid();
  const directoryCount = bytes.readUInt32LE(optional + directoryOffset - 4);
  if (directoryCount > Math.floor((optionalSize - directoryOffset) / 8)) invalid();
  const headersSize = bytes.readUInt32LE(optional + 60);
  const sectionTable = optional + optionalSize;
  range(sectionTable, sectionCount * 40);
  if (headersSize < sectionTable + sectionCount * 40 || headersSize > bytes.length) invalid();

  const sections = [];
  for (let index = 0; index < sectionCount; index += 1) {
    const at = sectionTable + index * 40;
    const size = bytes.readUInt32LE(at + 16);
    const raw = bytes.readUInt32LE(at + 20);
    const rva = bytes.readUInt32LE(at + 12);
    if (size !== 0) {
      if (raw < headersSize || rva < headersSize) invalid();
      range(raw, size);
      sections.push({ raw, rva, size });
    }
  }

  function rvaRange(rva, size) {
    const matches = sections.filter(section => rva >= section.rva
      && rva + size <= section.rva + section.size);
    if (rva < headersSize && rva + size <= headersSize) {
      if (matches.length !== 0) invalid();
      return { offset: rva, end: headersSize };
    }
    if (matches.length !== 1) invalid();
    const section = matches[0];
    return { offset: section.raw + rva - section.rva, end: section.raw + section.size };
  }

  function readName(rva) {
    if (rva === 0) invalid();
    const { offset, end } = rvaRange(rva, 1);
    const limit = Math.min(end, offset + 512);
    const nul = bytes.subarray(offset, limit).indexOf(0);
    if (nul <= 0) invalid();
    const name = bytes.subarray(offset, offset + nul);
    if (name.some(byte => byte < 0x21 || byte > 0x7e)) invalid();
    return name.toString("ascii");
  }

  function directory(index) {
    if (index >= directoryCount) return { rva: 0, size: 0 };
    const at = optional + directoryOffset + index * 8;
    const rva = bytes.readUInt32LE(at);
    const size = bytes.readUInt32LE(at + 4);
    if ((rva === 0) !== (size === 0)) invalid();
    return { rva, size };
  }

  const imageBase = magic === 0x20b
    ? bytes.readBigUInt64LE(optional + 24)
    : BigInt(bytes.readUInt32LE(optional + 28));
  function readImports(index, stride, nameOffset, delayed) {
    const { rva, size } = directory(index);
    if (rva === 0) return [];
    if (size < stride) invalid();
    // Validate the whole directory, not just a prefix that happens to terminate.
    rvaRange(rva, size);
    const names = [];
    for (let entry = 0; entry < Math.min(Math.floor(size / stride), 4096); entry += 1) {
      const { offset } = rvaRange(rva + entry * stride, stride);
      if (bytes.subarray(offset, offset + stride).every(byte => byte === 0)) return names;
      let nameRva = bytes.readUInt32LE(offset + nameOffset);
      if (delayed) {
        const attributes = bytes.readUInt32LE(offset);
        if (attributes !== 0 && attributes !== 1) invalid();
        if (attributes === 0) {
          // Legacy delay imports store a VA instead of an RVA.
          const relative = BigInt(nameRva) - imageBase;
          if (relative < 0n || relative > 0xffffffffn) invalid();
          nameRva = Number(relative);
        }
      }
      names.push(readName(nameRva));
    }
    invalid(); // Missing terminator or unreasonable descriptor count.
  }

  return {
    machine: bytes.readUInt16LE(pe + 4),
    format: magic === 0x20b ? "PE32+" : "PE32",
    subsystem: bytes.readUInt16LE(optional + 68),
    imports: [...readImports(1, 20, 12, false), ...readImports(13, 32, 4, true)],
  };
}

export function isDynamicMsvcCrtImport(name) {
  // Include debug/numbered variants and all UCRT API sets, not just MSVCP140.dll.
  return /^(?:(?:msvcp|msvcr|vcruntime|concrt|vcomp|vccorlib)\d[\w-]*|ucrtbased?|api-ms-win-crt-[\w-]+)\.dll$/i.test(name);
}
