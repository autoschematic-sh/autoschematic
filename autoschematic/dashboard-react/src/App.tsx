import { BrowserRouter as Router, Routes, Route, Navigate } from 'react-router-dom';
import './App.css';
import Header from './components/Header';
import DashboardList from './components/DashboardList';
import RepoView from './components/RepoView';

// Shoelace configuration
import '@shoelace-style/shoelace/dist/themes/light.css';
import { setBasePath } from '@shoelace-style/shoelace/dist/utilities/base-path.js';

setBasePath('https://cdn.jsdelivr.net/npm/@shoelace-style/shoelace@2.20.1/cdn/');

function App() {
  return (
    <Router>
      <div className="app-container h-screen flex flex-col">
        <Routes>
          <Route 
            path="/" 
            element={
              <>
                <Header />
                <main className="flex-grow overflow-auto">
                  <DashboardList />
                </main>
              </>
            } 
          />
          <Route 
            path="/repo/:owner/:repo/:installationId" 
            element={
              <>
                <Header owner="" repo="" />
                <main className="flex-grow overflow-auto">
                  <RepoView />
                </main>
              </>
            }
          />
          {/* Redirect all other paths to home */}
          <Route path="*" element={<Navigate to="/" replace />} />
        </Routes>
      </div>
    </Router>
  );
}

export default App;
