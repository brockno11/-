export function documentTitle(plainText: string): string {
  const firstLine = plainText
    .split(/\r?\n/)
    .map((line) => line.trim())
    .find(Boolean)
  return (firstLine || '新便签').slice(0, 80)
}
