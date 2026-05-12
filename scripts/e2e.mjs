import { spawn } from 'node:child_process'
import fs from 'node:fs'
import net from 'node:net'
import os from 'node:os'
import path from 'node:path'
import { fileURLToPath } from 'node:url'

const root = path.resolve(fileURLToPath(new URL('..', import.meta.url)))
const port = Number.parseInt(process.env.LHC_TAURI_DRIVER_PORT || '4444', 10)
const target = process.env.LHC_E2E_TARGET || 'desktop'
const knownTargets = new Set(['desktop', 'kde-wayland', 'windows'])
const devDir =
  process.env.LHC_DEV_DIR || fs.mkdtempSync(path.join(os.tmpdir(), 'lhc-e2e-'))
const appName = process.platform === 'win32' ? 'left-hand-control.exe' : 'left-hand-control'
const appPath =
  process.env.LHC_E2E_APP ||
  path.join(root, 'src-tauri', 'target', 'debug', appName)

const env = {
  ...process.env,
  LHC_DEV_DIR: devDir,
  LHC_E2E_APP: appPath,
  LHC_E2E_TARGET: target,
  LHC_TAURI_DRIVER_PORT: String(port),
}

if (!knownTargets.has(target)) {
  console.error(
    `Unknown LHC_E2E_TARGET="${target}". Expected one of: ${[...knownTargets].join(', ')}`,
  )
  process.exit(1)
}

if (target === 'windows' && process.platform !== 'win32') {
  console.error('LHC_E2E_TARGET=windows must run on a Windows desktop VM.')
  process.exit(1)
}

if (target === 'kde-wayland' && process.platform !== 'linux') {
  console.error('LHC_E2E_TARGET=kde-wayland must run on a Linux KDE Wayland desktop VM.')
  process.exit(1)
}

function run(command, args, options = {}) {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, {
      cwd: root,
      env,
      stdio: 'inherit',
      shell: process.platform === 'win32',
      ...options,
    })
    child.on('error', reject)
    child.on('exit', (code, signal) => {
      if (signal) {
        reject(new Error(`${command} ${args.join(' ')} exited via ${signal}`))
        return
      }
      if (code === 0) {
        resolve()
        return
      }
      reject(new Error(`${command} ${args.join(' ')} exited with code ${code}`))
    })
  })
}

async function commandWorks(command, args) {
  return new Promise((resolve) => {
    const child = spawn(command, args, {
      cwd: root,
      env,
      stdio: 'ignore',
      shell: process.platform === 'win32',
    })
    child.on('error', () => resolve(false))
    child.on('exit', (code) => resolve(code === 0))
  })
}

async function preflight() {
  if (!Number.isInteger(port) || port <= 0 || port > 65535) {
    throw new Error(`Invalid LHC_TAURI_DRIVER_PORT value: ${process.env.LHC_TAURI_DRIVER_PORT}`)
  }

  const hasDriver = await commandWorks('tauri-driver', ['--version'])
  if (!hasDriver) {
    throw new Error('tauri-driver is required. Install it with: cargo install tauri-driver --locked')
  }

  if (target === 'kde-wayland') {
    if (process.env.XDG_SESSION_TYPE !== 'wayland') {
      throw new Error(
        `KDE E2E must run inside a Wayland session. XDG_SESSION_TYPE=${process.env.XDG_SESSION_TYPE || '<empty>'}`,
      )
    }
    if (!String(process.env.XDG_CURRENT_DESKTOP || '').toLowerCase().includes('kde')) {
      throw new Error(
        `KDE E2E must run inside Plasma. XDG_CURRENT_DESKTOP=${process.env.XDG_CURRENT_DESKTOP || '<empty>'}`,
      )
    }
  }
}

function waitForTcp(host, tcpPort, timeoutMs = 15000) {
  const deadline = Date.now() + timeoutMs
  return new Promise((resolve, reject) => {
    const attempt = () => {
      const socket = net.createConnection({ host, port: tcpPort })
      socket.once('connect', () => {
        socket.end()
        resolve()
      })
      socket.once('error', () => {
        socket.destroy()
        if (Date.now() >= deadline) {
          reject(new Error(`tauri-driver did not open ${host}:${tcpPort}`))
          return
        }
        setTimeout(attempt, 200)
      })
    }
    attempt()
  })
}

function startTauriDriver() {
  const child = spawn('tauri-driver', ['--port', String(port)], {
    cwd: root,
    env,
    stdio: ['ignore', 'inherit', 'inherit'],
    shell: process.platform === 'win32',
  })
  child.on('error', (error) => {
    console.error(
      `Failed to start tauri-driver. Install it with: cargo install tauri-driver --locked\n${error}`,
    )
    process.exit(1)
  })
  return child
}

let driver
function stopDriver() {
  if (!driver || driver.killed) return
  driver.kill()
}

process.on('exit', stopDriver)
process.on('SIGINT', () => {
  stopDriver()
  process.exit(130)
})
process.on('SIGTERM', () => {
  stopDriver()
  process.exit(143)
})

console.log(`E2E target: ${target}`)
console.log(`E2E dev dir: ${devDir}`)

try {
  await preflight()

  if (process.env.LHC_E2E_SKIP_BUILD !== '1') {
    await run('pnpm', ['tauri', 'build', '--debug', '--no-bundle'])
  }

  if (!fs.existsSync(appPath)) {
    throw new Error(`Tauri debug binary not found: ${appPath}`)
  }

  driver = startTauriDriver()
  await waitForTcp('127.0.0.1', port)
  await run('pnpm', ['exec', 'wdio', 'run', 'e2e/wdio.conf.mjs'])
} catch (error) {
  stopDriver()
  console.error(error instanceof Error ? error.message : String(error))
  process.exit(1)
}
