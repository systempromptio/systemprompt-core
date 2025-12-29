export function parseDateTime(value: string | number | undefined): Date | undefined {
  if (!value) return undefined

  try {
    if (typeof value === 'number') {
      return new Date(value)
    }

    if (typeof value === 'string') {
      const asNumber = Number(value)
      if (!isNaN(asNumber)) {
        return new Date(asNumber)
      }

      const isoFormat = value.replace(' ', 'T')
      const date = new Date(isoFormat)

      if (!isNaN(date.getTime())) {
        return date
      }
    }

    return undefined
  } catch {
    return undefined
  }
}

export function parseToMs(value: string | number | Date | undefined): number {
  const date = value instanceof Date ? value : parseDateTime(value)
  return date ? date.getTime() : 0
}

export function elapsedMs(
  start: string | number | undefined,
  end: string | number | Date | undefined = new Date()
): number {
  const startMs = parseToMs(start)
  const endMs = parseToMs(end)

  if (startMs === 0) return 0

  return Math.max(0, endMs - startMs)
}

export function isPast(value: string | number | Date | undefined): boolean {
  const date = value instanceof Date ? value : parseDateTime(value)
  return date ? date.getTime() < Date.now() : false
}

export function isFuture(value: string | number | Date | undefined): boolean {
  const date = value instanceof Date ? value : parseDateTime(value)
  return date ? date.getTime() > Date.now() : false
}

export function isToday(value: string | number | Date | undefined): boolean {
  const date = value instanceof Date ? value : parseDateTime(value)
  if (!date) return false

  const today = new Date()
  return (
    date.getFullYear() === today.getFullYear() &&
    date.getMonth() === today.getMonth() &&
    date.getDate() === today.getDate()
  )
}

export function formatAsDate(
  value: string | number | Date | undefined,
  options?: Intl.DateTimeFormatOptions
): string {
  const date = value instanceof Date ? value : parseDateTime(value)
  if (!date) return ''

  return date.toLocaleDateString(undefined, options)
}

export function formatAsDateTime(
  value: string | number | Date | undefined,
  options?: Intl.DateTimeFormatOptions
): string {
  const date = value instanceof Date ? value : parseDateTime(value)
  if (!date) return ''

  return date.toLocaleString(undefined, options)
}

export function formatAsTime(
  value: string | number | Date | undefined,
  options?: Intl.DateTimeFormatOptions
): string {
  const date = value instanceof Date ? value : parseDateTime(value)
  if (!date) return ''

  return date.toLocaleTimeString(undefined, options)
}

export function formatAsRelative(value: string | number | Date | undefined): string {
  const date = value instanceof Date ? value : parseDateTime(value)
  if (!date) return ''

  const now = Date.now()
  const diff = date.getTime() - now

  if (diff === 0) return 'now'

  const absMs = Math.abs(diff)
  const sign = diff < 0 ? -1 : 1

  const seconds = Math.floor(absMs / 1000)
  const minutes = Math.floor(seconds / 60)
  const hours = Math.floor(minutes / 60)
  const days = Math.floor(hours / 24)
  const weeks = Math.floor(days / 7)
  const months = Math.floor(days / 30)
  const years = Math.floor(days / 365)

  try {
    const rtf = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' })

    if (years !== 0) return rtf.format(sign * years, 'year')
    if (months !== 0) return rtf.format(sign * months, 'month')
    if (weeks !== 0) return rtf.format(sign * weeks, 'week')
    if (days !== 0) return rtf.format(sign * days, 'day')
    if (hours !== 0) return rtf.format(sign * hours, 'hour')
    if (minutes !== 0) return rtf.format(sign * minutes, 'minute')

    return rtf.format(sign * seconds, 'second')
  } catch {
    return ''
  }
}

export function getStartOfDay(value?: string | number | Date): Date {
  const date = value ? (value instanceof Date ? value : parseDateTime(value)) : new Date()
  if (!date) return new Date()

  const start = new Date(date)
  start.setHours(0, 0, 0, 0)
  return start
}

export function getEndOfDay(value?: string | number | Date): Date {
  const date = value ? (value instanceof Date ? value : parseDateTime(value)) : new Date()
  if (!date) return new Date()

  const end = new Date(date)
  end.setHours(23, 59, 59, 999)
  return end
}

export function isSameDay(
  date1: string | number | Date | undefined,
  date2: string | number | Date | undefined
): boolean {
  const d1 = date1 instanceof Date ? date1 : parseDateTime(date1)
  const d2 = date2 instanceof Date ? date2 : parseDateTime(date2)

  if (!d1 || !d2) return false

  return (
    d1.getFullYear() === d2.getFullYear() &&
    d1.getMonth() === d2.getMonth() &&
    d1.getDate() === d2.getDate()
  )
}
