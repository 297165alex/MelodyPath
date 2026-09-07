// Music desktop exports may use UTF-16; do not silently replace broken text.
export async function readPlaylistFile(file: File): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer())
  const encoding = bytes[0] === 0xff && bytes[1] === 0xfe ? 'utf-16le'
    : bytes[0] === 0xfe && bytes[1] === 0xff ? 'utf-16be' : 'utf-8'
  try { return new TextDecoder(encoding, { fatal: true }).decode(bytes) }
  catch { throw new Error('文件编码无效，请另存为 UTF-8 或带 BOM 的 UTF-16 文本后导入。') }
}
