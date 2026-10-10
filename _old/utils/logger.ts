type LogLevel = 'debug' | 'info' | 'warn' | 'error'

function serializeCtx(ctx: unknown): string {
  if (ctx instanceof Error) return ctx.stack ?? ctx.message
  try {
    return JSON.stringify(ctx)
  } catch {
    return String(ctx)
  }
}

function entry(level: LogLevel, message: string, context?: unknown): string {
  const ts = new Date().toISOString()
  const prefix = `[${ts}] [${level.toUpperCase()}]`
  return context !== undefined
    ? `${prefix} ${message} — ${serializeCtx(context)}`
    : `${prefix} ${message}`
}

export const logger = {
  debug(message: string, context?: unknown): void {
    if (import.meta.env.PROD) return
    console.debug(entry('debug', message, context))
  },
  info(message: string, context?: unknown): void {
    console.info(entry('info', message, context))
  },
  warn(message: string, context?: unknown): void {
    console.warn(entry('warn', message, context))
  },
  error(message: string, context?: unknown): void {
    console.error(entry('error', message, context))
  },
}
