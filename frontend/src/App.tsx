import { useEffect, useState } from 'react'
import { HashRouter, Navigate, Route, Routes } from 'react-router-dom'

import { fetchDashboard, fetchHealth, fetchPeople, fetchUnmatchedAuthors } from './api'
import { Sidebar } from './components/layout/Sidebar.tsx'
import { Toast } from './components/layout/Toast.tsx'
import { ToastProvider } from './context/ToastContext.tsx'
import { DashboardPage } from './pages/DashboardPage.tsx'
import { MrInboxPage } from './pages/MrInboxPage.tsx'
import { PeoplePage } from './pages/PeoplePage.tsx'
import { ProjectsPage } from './pages/ProjectsPage.tsx'
import { ReportsPage } from './pages/ReportsPage.tsx'
import { RunsPage } from './pages/RunsPage.tsx'
import type { Person } from './types'

export function App() {
  return (
    <HashRouter>
      <ToastProvider>
        <AppShell />
      </ToastProvider>
    </HashRouter>
  )
}

function AppShell() {
  const [statusLine, setStatusLine] = useState('連線中...')
  const [people, setPeople] = useState<Person[]>([])
  const [mrDraftCount, setMrDraftCount] = useState(0)
  const [unmatchedCount, setUnmatchedCount] = useState(0)

  useEffect(() => {
    let cancelled = false

    async function loadShellData() {
      try {
        const [health, dashboard, peopleResponse, unmatched] = await Promise.all([
          fetchHealth(),
          fetchDashboard(),
          fetchPeople(),
          fetchUnmatchedAuthors().catch(() => []),
        ])
        if (cancelled) return
        setStatusLine(`已連線 · ${health.data_dir}`)
        setMrDraftCount(dashboard.stats.mr_draft_count)
        setPeople(peopleResponse)
        setUnmatchedCount(unmatched.length)
      } catch (error) {
        if (!cancelled) {
          setStatusLine(error instanceof Error ? `連線失敗 · ${error.message}` : '連線失敗')
        }
      }
    }

    void loadShellData()
    return () => {
      cancelled = true
    }
  }, [])

  return (
    <div className="flex min-h-full bg-page text-ink font-sans">
      <Sidebar
        statusLine={statusLine}
        mrDraftCount={mrDraftCount}
        unmatchedCount={unmatchedCount}
        people={people}
      />
      <div className="flex min-w-0 flex-1 flex-col">
        <Toast />
        <main className="min-w-0 flex-1 px-10 py-8">
          <Routes>
            <Route path="/" element={<Navigate to="/dashboard" replace />} />
            <Route path="/dashboard" element={<DashboardPage />} />
            <Route path="/runs" element={<RunsPage />} />
            <Route path="/runs/:runId" element={<RunsPage />} />
            <Route path="/mr-inbox" element={<MrInboxPage />} />
            <Route path="/reports" element={<ReportsPage />} />
            <Route path="/reports/:personId" element={<ReportsPage />} />
            <Route path="/projects" element={<ProjectsPage />} />
            <Route path="/people" element={<PeoplePage />} />
          </Routes>
        </main>
      </div>
    </div>
  )
}
