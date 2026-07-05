import './style.css'
import { ReviewerApp } from './app'

const root = document.querySelector<HTMLDivElement>('#app')
if (!root) {
  throw new Error('missing #app root element')
}

const app = new ReviewerApp(root)
void app.init()
