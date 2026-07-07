import { apiUrl as joinApiUrl, normalizeApiBase } from '../base'

export const API_BASE = normalizeApiBase(import.meta.env.VITE_API_BASE)

export function apiUrl(path: string): string {
  return joinApiUrl(API_BASE, path)
}
