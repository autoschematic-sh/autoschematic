import React from 'react';

interface HeaderProps {
  owner?: string;
  repo?: string;
}

/**
 * Application header component
 * Displays the logo and repository info when available
 */
const Header: React.FC<HeaderProps> = ({ owner, repo }) => {
  return (
    <header className="bg-gray-200 py-4 px-6 flex items-center justify-between">
      <div className="flex items-center">
        <span className="text-2xl font-bold">Logo</span>
        {owner && repo && (
          <h3 className="text-2xl font-mono ml-4">{owner}/{repo}</h3>
        )}
      </div>
      <h1 className="text-2xl font-mono">autoschematic</h1>
    </header>
  );
};

export default Header;
