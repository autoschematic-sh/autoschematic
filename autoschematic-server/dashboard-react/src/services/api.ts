import {
  mockInstallations,
  mockRepoDetails,
  shouldUseMockData,
  Installation,
  RepoDetails,
  PrefixListing,
  TaskListing
} from './mockData';

/**
 * API Service for interacting with backend endpoints
 */
export class ApiService {
  /**
   * Get list of installations for the current user
   */
  static async getInstallations(): Promise<Installation[]> {
    // Use mock data in development
    if (shouldUseMockData()) {
      console.log('Using mock installation data');
      return Promise.resolve(mockInstallations);
    }

    try {
      const response = await fetch('/api/repo/');

      if (response.status == 401) {
        window.location.href = '/api/login';
        return [];
      }
      // Check if the response is a redirect to login
      if (response.redirected && response.url.includes('/api/login')) {
        window.location.href = '/api/login';
        return [];
      }

      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }

      // For this implementation, we're returning a direct API response
      // In a real implementation, you'd transform the HTML or use a separate JSON endpoint
      const data = await response.json();
      console.log(response);
      //   console.log(response);
      return data || [];
    } catch (error) {
      console.error('Error fetching installations:', error);
      return [];
    }
  }

  /**
   * Get repo view data including prefixes
   */
  static async getRepoDetails(owner: string, repo: string, installationId: string): Promise<RepoDetails> {
    // Use mock data in development
    if (shouldUseMockData()) {
      console.log('Using mock repo details data');
      return Promise.resolve({
        ...mockRepoDetails,
        owner,
        repo,
        installationId
      });
    }

    try {
      const url = `/api/repo/${owner}/${repo}/${installationId}/view`;
      const response = await fetch(url);

      if (response.status == 401) {
        window.location.href = '/api/login';
        return { owner, repo, installationId, prefixListings: [] };
      }

      if (response.redirected && response.url.includes('/api/login')) {
        window.location.href = '/api/login';
        return { owner, repo, installationId, prefixListings: [] };
      }

      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }

      // In a real implementation, this would transform server response or use a JSON endpoint
      const data = await response.json();
      console.log(data)
      return {
        owner,
        repo,
        installationId,
        prefixListings: data || []
      };
    } catch (error) {
      console.error('Error fetching repo details:', error);
      return { owner, repo, installationId, prefixListings: [] };
    }
  }

  /**
   * Spawn a task
   */
  static async spawnTask(owner: string, repo: string, installationId: string, prefix: string, agentName: string, arg: any): Promise<boolean> {
    // Simulate spawning in development mode
    if (shouldUseMockData()) {
      console.log(`Mock spawning agent: ${prefix}/${agentName}`);
      // Simulate a slight delay
      await new Promise(resolve => setTimeout(resolve, 700));
      return true;
    }

    try {
      const url = `/api/repo/${owner}/${repo}/${installationId}/${encodeURIComponent(prefix)}/${encodeURIComponent(agentName)}/spawn`;
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: arg
      });

      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }

      return true;
    } catch (error) {
      console.error('Error spawning task:', error);
      return false;
    }
  }

  /**
   * Send a task message
   */
  static async sendTaskMessage(owner: string, repo: string, installationId: string, prefix: string, taskName: string, message: any): Promise<boolean> {
    // Simulate spawning in development mode
    if (shouldUseMockData()) {
      console.log(`Mock spawning task: ${prefix}/${taskName}`);
      // Simulate a slight delay
      await new Promise(resolve => setTimeout(resolve, 700));
      return true;
    }

    try {
      const url = `/api/repo/${owner}/${repo}/${installationId}/${encodeURIComponent(prefix)}/${encodeURIComponent(taskName)}/send`;
      const response = await fetch(url, {
        method: 'POST',
        headers: {
          'Content-Type': 'application/json'
        },
        body: JSON.stringify(message)
      });

      if (!response.ok) {
        throw new Error(`API error: ${response.statusText}`);
      }

      return true;
    } catch (error) {
      console.error('Error sending task message:', error);
      return false;
    }
  }
}

// Re-export types for convenience
export type { Installation, RepoDetails, PrefixListing, TaskListing };
