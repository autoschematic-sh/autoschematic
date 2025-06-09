/**
 * Mock data for development and testing
 * This allows us to see the dashboard without a real backend response
 */

export interface Installation {
  owner: string;
  repo: string;
  installation_id: string;
}

export interface TaskListing {
  name: string;
  state?: any;
}

export interface PrefixListing {
  name: string;
  tasks: TaskListing[];
}

export interface RepoDetails {
  owner: string;
  repo: string;
  installationId: string;
  prefixListings: PrefixListing[];
}

export const mockInstallations: Installation[] = [
  {
    owner: 'user1',
    repo: 'project-alpha',
    installation_id: '12345'
  },
  {
    owner: 'user2',
    repo: 'project-beta',
    installation_id: '67890'
  },
  {
    owner: 'org',
    repo: 'enterprise-project',
    installation_id: '54321'
  }
];

export const mockPrefixListings: PrefixListing[] = [
  {
    name: 'test_prefix',
    tasks: [
      { name: 'agent1', state: null },
      { name: 'agent2', state: { status: 'idle' } }
    ]
  },
  {
    name: 'aws/iam',
    tasks: [
      { name: 'policy-agent', state: null },
      { name: 'role-agent', state: null },
      { name: 'user-agent', state: { status: 'running' } }
    ]
  },
  {
    name: 'gcp',
    tasks: [
      { name: 'compute-agent', state: null },
      { name: 'storage-agent', state: null }
    ]
  }
];

export const mockRepoDetails: RepoDetails = {
  owner: 'user1',
  repo: 'project-alpha',
  installationId: '12345',
  prefixListings: mockPrefixListings
};

/**
 * Use this function to determine if we should use mock data
 * During development, this can be set to true to use mock data
 */
export function shouldUseMockData(): boolean {
  // Check for a development parameter or environment
  const urlParams = new URLSearchParams(window.location.search);
  return urlParams.has('mock') || 
         window.location.hostname === 'localhost' ||
         window.location.hostname === '127.0.0.1';
}
