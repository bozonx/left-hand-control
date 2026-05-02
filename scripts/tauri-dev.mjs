import { spawn } from 'node:child_process'

const port = process.env.PORT || '3000'
const devUrl = `http://localhost:${port}`

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

const child = spawn('tauri', ['dev', '--config', JSON.stringify(config)], {
  stdio: 'inherit',
  shell: false,
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
