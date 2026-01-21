<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { auth } from '$lib/auth.svelte';
	import { getServers, createServer, type Server } from '$lib/api';

	let { children } = $props();

	let servers = $state<Server[]>([]);
	let loading = $state(true);
	let showCreateModal = $state(false);
	let newServerName = $state('');
	let creating = $state(false);

	onMount(async () => {
		await auth.init();
		if (!auth.isAuthenticated) {
			goto('/');
			return;
		}
		await loadServers();
	});

	async function loadServers() {
		loading = true;
		const result = await getServers();
		if (result.data) {
			servers = result.data;
		}
		loading = false;
	}

	async function handleCreateServer(e: Event) {
		e.preventDefault();
		if (!newServerName.trim()) return;

		creating = true;
		const result = await createServer(newServerName.trim());
		creating = false;

		if (result.data) {
			servers = [...servers, result.data];
			newServerName = '';
			showCreateModal = false;
			goto(`/app/${result.data.id}`);
		}
	}

	function getServerInitial(name: string): string {
		return name.charAt(0).toUpperCase();
	}
</script>

<div class="app-layout">
	<aside class="server-bar">
		{#if loading}
			<div class="server-icon loading"></div>
		{:else}
			{#each servers as server}
				<a href="/app/{server.id}" class="server-icon" title={server.name}>
					{getServerInitial(server.name)}
				</a>
			{/each}
		{/if}

		<button class="server-icon add-server" title="Create Server" onclick={() => (showCreateModal = true)}>
			<span>+</span>
		</button>

		<div class="server-bar-spacer"></div>

		<button class="server-icon logout" title="Logout" onclick={() => { auth.logout(); goto('/'); }}>
			←
		</button>
	</aside>

	<div class="app-content">
		{@render children()}
	</div>
</div>

{#if showCreateModal}
	<div class="modal-backdrop" onclick={() => (showCreateModal = false)}>
		<div class="modal" onclick={(e) => e.stopPropagation()}>
			<h2>Create a Server</h2>
			<form onsubmit={handleCreateServer}>
				<div class="form-group">
					<label for="server-name">Server Name</label>
					<input
						type="text"
						id="server-name"
						bind:value={newServerName}
						placeholder="My Awesome Server"
						required
					/>
				</div>
				<div class="modal-actions">
					<button type="button" class="secondary" onclick={() => (showCreateModal = false)}>
						Cancel
					</button>
					<button type="submit" class="primary" disabled={creating}>
						{creating ? 'Creating...' : 'Create'}
					</button>
				</div>
			</form>
		</div>
	</div>
{/if}

<style>
	.app-layout {
		display: flex;
		height: 100vh;
		overflow: hidden;
	}

	.server-bar {
		width: 72px;
		background-color: var(--bg-tertiary);
		display: flex;
		flex-direction: column;
		align-items: center;
		padding: 12px 0;
		gap: 8px;
		overflow-y: auto;
	}

	.server-icon {
		width: 48px;
		height: 48px;
		border-radius: 50%;
		background-color: var(--bg-secondary);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 18px;
		color: var(--text-primary);
		cursor: pointer;
		transition: border-radius 0.2s, background-color 0.2s;
		text-decoration: none;
		border: none;
	}

	.server-icon:hover {
		border-radius: 16px;
		background-color: var(--accent);
	}

	.server-icon.add-server {
		background-color: transparent;
		border: 2px dashed var(--border);
		color: var(--accent);
		font-size: 24px;
	}

	.server-icon.add-server span {
		transform: translateY(-2px);
	}

	.server-icon.add-server:hover {
		border-color: var(--accent);
		background-color: transparent;
	}

	.server-icon.logout {
		background-color: transparent;
		color: var(--text-muted);
		font-size: 20px;
	}

	.server-icon.logout:hover {
		color: var(--danger);
		background-color: transparent;
	}

	.server-icon.loading {
		background: linear-gradient(90deg, var(--bg-secondary) 25%, var(--bg-primary) 50%, var(--bg-secondary) 75%);
		background-size: 200% 100%;
		animation: shimmer 1.5s infinite;
	}

	@keyframes shimmer {
		0% { background-position: 200% 0; }
		100% { background-position: -200% 0; }
	}

	.server-bar-spacer {
		flex: 1;
	}

	.app-content {
		flex: 1;
		display: flex;
		overflow: hidden;
	}

	/* Modal styles */
	.modal-backdrop {
		position: fixed;
		inset: 0;
		background: rgba(0, 0, 0, 0.7);
		display: flex;
		align-items: center;
		justify-content: center;
		z-index: 100;
	}

	.modal {
		background: var(--bg-secondary);
		border-radius: 8px;
		padding: 24px;
		width: 400px;
		max-width: 90vw;
	}

	.modal h2 {
		margin: 0 0 20px;
		font-size: 20px;
	}

	.form-group {
		margin-bottom: 16px;
	}

	.form-group label {
		display: block;
		font-size: 12px;
		text-transform: uppercase;
		color: var(--text-muted);
		margin-bottom: 8px;
		letter-spacing: 0.5px;
	}

	.form-group input {
		width: 100%;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 12px;
		margin-top: 20px;
	}
</style>
