# Autoschematic Dashboard (React)

This is a React port of the original Tailwind CSS and server-side templating dashboard. It provides the same functionality but implemented with modern React practices.

## Features

- Lists repositories with active runs
- Shows prefixes for selected repository 
- Allows spawning of tasks
- Uses Tailwind CSS for styling
- Includes Shoelace web components
- Provides mock data for local development

## Project Structure

- `/src/components/` - React components
  - `Header.tsx` - App header with logo and repo info
  - `DashboardList.tsx` - List of installations
  - `RepoView.tsx` - Repository view with task listing
- `/src/services/` - API and data services
  - `api.ts` - Service for API calls
  - `mockData.ts` - Mock data for development

## Development

To start the development server:

```bash
# Install dependencies
npm install

# Generate Tailwind CSS
npm run tailwind:build

# Run development server with Tailwind watcher
npm run start
```

## Building

To build for production:

```bash
npm run build
```

## API Endpoints

The dashboard uses the following API endpoints:

- GET `/api/repo/` - List installations
- GET `/api/repo/{owner}/{repo}/{installation_id}/view` - Get repository details
- POST `/api/repo/{owner}/{repo}/{installation_id}/{prefix}/{task}/spawn` - Spawn a task
