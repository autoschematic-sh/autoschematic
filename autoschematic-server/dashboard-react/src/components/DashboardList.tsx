import React, { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import { ApiService, Installation } from '../services/api';
// import SlButton from '@shoelace-style/shoelace/dist/react/button/index.js';
import SlSpinner from '@shoelace-style/shoelace/dist/react/spinner/index.js';



/**
 * Dashboard list component
 * Displays a list of all active installations
 */
const DashboardList: React.FC = () => {
  const [installations, setInstallations] = useState<Installation[]>([]);
  const [loading, setLoading] = useState<boolean>(true);
  const [error, setError] = useState<string | null>(null);
  const navigate = useNavigate();

  useEffect(() => {
    loadInstallations();
  }, []);

  async function loadInstallations() {
    try {
      setLoading(true);
      const data = await ApiService.getInstallations();
      setInstallations(data);
      console.log("Got installations", data);
    } catch (err) {
      const errorMessage = err instanceof Error ? err.message : 'Unknown error loading installations';
      setError(errorMessage);
      console.error('Error loading installations:', err);
    } finally {
      setLoading(false);
    }
  }

  const handleClick = (e: React.MouseEvent<HTMLAnchorElement>, install: Installation) => {
    e.preventDefault();
    navigate(`/repo/${install.owner}/${install.repo}/${install.installation_id}`);
  };

  const renderInstallationItem = (install: Installation) => {
    const url = `/repo/${install.owner}/${install.repo}/${install.installation_id}`;
    
    return (
      <li key={install.installation_id} className="py-2 border-b border-gray-200">
        <a 
          href={url}
          className="w-full inline-flex items-center justify-center p-5 text-base font-medium text-gray-500 rounded-lg bg-gray-50 hover:text-gray-900 hover:bg-gray-100 dark:text-gray-400 dark:bg-gray-800 dark:hover:bg-gray-700 dark:hover:text-white"
          onClick={(e) => handleClick(e, install)}
        >
          <span className="font-mono">{install.owner}/{install.repo}</span>
        </a>
      </li>
    );
  };

  const renderInstallationList = () => {
    if (error) {
      return <div className="text-red-500">{error}</div>;
    }

    if (installations.length === 0) {
      return <div className="py-4">No active installations found.</div>;
    }

    return (
      <ul className="list-none p-0">
        {installations.map(install => renderInstallationItem(install))}
      </ul>
    );
  };

  return (
    <div className="flex items-center justify-center h-full">
      <div className="bg-white shadow-md rounded px-8 pt-6 pb-8 mb-4 w-full max-w-4xl">
        <h1 className="text-2xl font-bold mb-6">Active Runs</h1>
        
        {loading 
          ? <SlSpinner></SlSpinner>
          : renderInstallationList()
        }
      </div>
    </div>
  );
};

export default DashboardList;
