/** Vite asset base, e.g. `/reviewer/` when hosted under a subpath. */
export function normalizeBasePath(raw: string | undefined): string {
  const value = raw?.trim()
  if (!value || value === '/') {
    return '/'
  }
  const withLeading = value.startsWith('/') ? value : `/${value}`
  return withLeading.endsWith('/') ? withLeading : `${withLeading}/`
}

/** API prefix before `/health` and `/api/*`. Empty = site root. */
export function normalizeApiBase(raw: string | undefined): string {
  const value = raw?.trim() ?? ''
  if (!value || value === '/') {
    return ''
  }
  const withLeading = value.startsWith('/') ? value : `/${value}`
  return withLeading.endsWith('/') ? withLeading.slice(0, -1) : withLeading
}

export function apiUrl(apiBase: string, path: string): string {
  return `${apiBase}${path}`
}
