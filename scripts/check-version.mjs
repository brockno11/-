import { readFileSync } from 'node:fs'

const packageVersion = JSON.parse(readFileSync('package.json', 'utf8')).version
const tauriVersion = JSON.parse(readFileSync('src-tauri/tauri.conf.json', 'utf8')).version
const cargoText = readFileSync('src-tauri/Cargo.toml', 'utf8')
const cargoVersion = cargoText.match(/^version\s*=\s*"([^"]+)"/m)?.[1]
const versions = new Map([
  ['package.json', packageVersion],
  ['src-tauri/tauri.conf.json', tauriVersion],
  ['src-tauri/Cargo.toml', cargoVersion],
])

const expected = packageVersion
const mismatches = [...versions].filter(([, version]) => version !== expected)
if (mismatches.length) {
  console.error('版本号不一致：')
  for (const [file, version] of versions) console.error(`- ${file}: ${version ?? '未找到'}`)
  process.exit(1)
}

const tag = process.env.GITHUB_REF_NAME
if (tag?.startsWith('v') && tag.slice(1) !== expected) {
  console.error(`Git 标签 ${tag} 与应用版本 ${expected} 不一致`)
  process.exit(1)
}

console.log(`版本一致：${expected}`)
