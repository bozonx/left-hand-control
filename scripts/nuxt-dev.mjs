import { spawn } from 'node:child_process'
import net from 'node:net'

const host = process.env.HOST || 'localhost'
const port = Number.parseInt(process.env.PORT || '3000', 10)

if (!Number.isInteger(port) || port <= 0 || port > 65535) {
  console.error(`Invalid PORT value: ${process.env.PORT}`)
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

const child = spawn('nuxt', ['dev', '--host', host, '--port', String(port)], {
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
