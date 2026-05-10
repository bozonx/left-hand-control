import { spawn } from 'node:child_process'

const port = process.env.LHC_DEV_PORT || '3000'
const devUrl = `http://localhost:${port}`

// Pass the resolved port down to Nuxt via `pnpm dev` so Tauri's beforeDevCommand
// reads the same value, regardless of whether `.env` was loaded by the parent.
process.env.LHC_DEV_PORT = port

const config = {
  build: {
    devUrl,
  },
  app: {
    security: {
      csp: [
        "default-src 'self' ipc: http://ipc.localhost",
        `connect-src 'self' ipc: http://ipc.localhost ${devUrl} ws://localhost:${port} https://api.iconify.design`,
        "img-src 'self' asset: http://asset.localhost blob: data:",
        "style-src 'self' 'unsafe-inline'",
        "font-src 'self' asset: http://asset.localhost data:",
        "script-src 'self' 'unsafe-inline' 'wasm-unsafe-eval'",
      ].join('; '),
    },
  },
}

// On Windows, the `tauri` binary is a `.cmd` shim that requires a shell.
const child = spawn('tauri', ['dev', '--config', JSON.stringify(config)], {
  stdio: 'inherit',
  shell: process.platform === 'win32',
})

child.on('exit', (code, signal) => {
  if (signal) {
    process.kill(process.pid, signal)
    return
  }

  process.exit(code ?? 1)
})

child.on('error', (error) => {
  console.error(error)
  process.exit(1)
})
