import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'
import { App } from './App.tsx'
import './index.css'

const root = document.querySelector<HTMLDivElement>('#app')
if (!root) {
  throw new Error('missing #app root element')
}

createRoot(root).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
