export function gitlabIconDataUri(hex: string): string {
  const encoded = hex.replace('#', '%23')
  return `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 256 256' fill='${encoded}'%3E%3Cpath d='M231.92773,169.78029l-94.82031,65.64454a16.07612,16.07612,0,0,1-18.21484,0L24.07227,169.78029a16.03981,16.03981,0,0,1-6.35254-17.27783L45.04883,50.0176a12.00012,12.00012,0,0,1,22.831-1.12109L88.544,104h78.9121l20.66407-55.10449a12.00021,12.00021,0,0,1,22.83056,1.12109l27.32959,102.48584A16.03981,16.03981,0,0,1,231.92773,169.78029Z'/%3E%3C/svg%3E")`
}

export function folderIconDataUri(hex: string): string {
  const encoded = hex.replace('#', '%23')
  return `url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 24 24' fill='${encoded}'%3E%3Cpath d='M10 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V8a2 2 0 0 0-2-2h-8l-2-2Z'/%3E%3C/svg%3E")`
}

export function sourceIconStyle(
  sourceType: 'gitlab' | 'local',
  active: boolean,
): { width: number; height: number; backgroundImage: string } {
  const color = active ? '#4f46e5' : '#94a3b8'
  return {
    width: 15,
    height: 15,
    backgroundImage: sourceType === 'gitlab' ? gitlabIconDataUri(color) : folderIconDataUri(color),
  }
}
