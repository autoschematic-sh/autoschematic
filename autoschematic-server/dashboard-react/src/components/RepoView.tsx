import React, { useState, useEffect } from 'react';
import { useParams } from 'react-router-dom';
import { ApiService, PrefixListing, TaskListing } from '../services/api';
import SlSpinner from '@shoelace-style/shoelace/dist/react/spinner/index.js';
import SlIconButton from '@shoelace-style/shoelace/dist/react/icon-button/index.js';

/**
 * Repository View Component
 * Shows tasks grouped by prefix with spawn functionality
 */
const RepoView: React.FC = () => {
    const { owner, repo, installationId } = useParams<{
        owner: string;
        repo: string;
        installationId: string;
    }>();

    const [prefixListings, setPrefixListings] = useState<PrefixListing[]>([]);
    const [loading, setLoading] = useState<boolean>(true);
    const [error, setError] = useState<string | null>(null);
    const [spawnLoading, setSpawnLoading] = useState<Record<string, boolean>>({});
    const [messageLoading, setMessageLoading] = useState<Record<string, boolean>>({});

    useEffect(() => {
        if (owner && repo && installationId) {
            loadRepoDetails();
        }
    }, [owner, repo, installationId]);

    async function loadRepoDetails() {
        if (!owner || !repo || !installationId) {
            return;
        }

        try {
            setLoading(true);
            setError(null);

            const details = await ApiService.getRepoDetails(
                owner,
                repo,
                installationId
            );
            setPrefixListings(details.prefixListings);
        } catch (err) {
            const errorMessage = err instanceof Error ? err.message : 'Unknown error loading repository details';
            setError(errorMessage);
            console.error('Error loading repository details:', err);
        } finally {
            setLoading(false);
        }
    }

    async function spawnTask(prefix: string, taskName: string) {
        if (!owner || !repo || !installationId) {
            return;
        }

        const spawnKey = `${prefix}:${taskName}`;
        setSpawnLoading(prev => ({ ...prev, [spawnKey]: true }));

        try {
            const result = await ApiService.spawnTask(
                owner,
                repo,
                installationId,
                prefix,
                taskName,
                0
            );

            if (result) {
                // Refresh the data after successful spawn
                loadRepoDetails();
            } else {
                throw new Error('Failed to spawn task');
            }
        } catch (err) {
            console.error('Error spawning task:', err);
            // Show a toast message or other notification here
        } finally {
            setSpawnLoading(prev => {
                const newState = { ...prev };
                delete newState[spawnKey];
                return newState;
            });
        }
    }

    async function sendTaskMessage(prefix: string, taskName: string, message: any) {
        if (!owner || !repo || !installationId) {
            return;
        }

        const messageKey = `${prefix}:${taskName}`;
        setMessageLoading(prev => ({ ...prev, [messageKey]: true }));

        try {
            const result = await ApiService.sendTaskMessage(
                owner,
                repo,
                installationId,
                prefix,
                taskName,
                message
            );

            if (result) {
                // Refresh the data after successful message
                loadRepoDetails();
            } else {
                throw new Error('Failed to send message to task');
            }
        } catch (err) {
            console.error('Error sending message to task:', err);
            // Show a toast message or other notification here
        } finally {
            setSpawnLoading(prev => {
                const newState = { ...prev };
                delete newState[messageKey];
                return newState;
            });
        }
    }

    const renderTaskListing = (prefixName: string, task: TaskListing) => {
        const spawnKey = `${prefixName}:${task.name}`;
        const isLoading = spawnLoading[spawnKey] || false;
        const messageKey = `${prefixName}:${task.name}`;
        const messageInFlight = messageLoading[messageKey] || false;
        console.log(task);
        const taskRunning = task.state?.type == "Running"

        return (
            <li key={task.name} className="py-2 border-b border-gray-200">
                <div className="flex items-center justify-between">
                    <span className="flex font-mono">{task.name} - {task.state?.type} </span>
                    {isLoading ? (
                        <SlSpinner style={{ fontSize: '1rem' }}></SlSpinner>
                    ) : (
                        <div>
                            <SlIconButton
                                name="stop"
                                label="Kill Task"
                                onClick={() => sendTaskMessage(prefixName, task.name, {type: "ShutDown"})}
                                className="flex bg-gray-300 hover:bg-gray-400 text-gray-800 font-bold py-2 px-4 rounded inline-flex items-center"
                                disabled={isLoading || (!taskRunning) || messageInFlight}
                            />
                            <SlIconButton
                                name="play"
                                label="Spawn Task"
                                onClick={() => spawnTask(prefixName, task.name)}
                                className="flex bg-gray-300 hover:bg-gray-400 text-gray-800 font-bold py-2 px-4 rounded inline-flex items-center"
                                disabled={isLoading || taskRunning}
                            />
                        </div>
                    )}
                </div>
            </li>
        );
    };

    const renderPrefixListing = (prefixListing: PrefixListing) => {
        return (
            <li key={prefixListing.name} className="py-2 border-b border-gray-200">
                <span className="font-mono">{prefixListing.name}</span>
                <ul className="list-none p-0">
                    {prefixListing.tasks.map(task => renderTaskListing(prefixListing.name, task))}
                </ul>
            </li>
        );
    };

    const renderPrefixListings = () => {
        console.log("renderPrefixListings: ", prefixListings);
        if (error) {
            return <div className="text-red-500">{error}</div>;
        }

        if (prefixListings.length === 0) {
            return <div className="py-4">No Autoschematic prefixes found for this repository.</div>;
        }

        return (
            <ul className="list-none p-0">
                {prefixListings.map(prefix => renderPrefixListing(prefix))}
            </ul>
        );
    };

    return (
        <div className="flex items-center justify-center h-full">
            <div className="bg-white shadow-md rounded px-8 pt-6 pb-8 mb-4 w-full max-w-4xl">
                <h1 className="text-2xl font-bold mb-6">Tasks by Prefix</h1>

                {loading ? (
                    <SlSpinner></SlSpinner>
                ) : (
                    renderPrefixListings()
                )}
            </div>
        </div>
    );
};

export default RepoView;
