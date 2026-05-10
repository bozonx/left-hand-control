import { spawn } from 'node:child_process'
import net from 'node:net'

const host = process.env.HOST || 'localhost'
const rawPort = process.env.LHC_DEV_PORT || '3000'
const port = Number.parseInt(rawPort, 10)

if (!Number.isInteger(port) || port <= 0 || port > 65535) {
  console.error(`Invalid LHC_DEV_PORT value: ${rawPort}`)
  process.exit(1)
}

function checkPortAvailable() {
  return new Promise((resolve, reject) => {
    const server = net.createServer()
    server.once('error', reject)
    server.once('listening', () => {
      server.close(() => resolve())
    })
    server.listen(port, host)
  })
}

try {
  await checkPortAvailable()
} catch {
  console.error(`Port ${port} on ${host} is already in use. Tauri devUrl requires a fixed port.`)
  process.exit(1)
}

// On Windows, npm/pnpm-installed binaries are `.cmd` shims that cannot be
// launched without a shell, so spawn through cmd.exe there.
const child = spawn('nuxt', ['dev', '--host', host, '--port', String(port)], {
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
