<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { auth } from '$lib/auth.svelte';
	import { websocket } from '$lib/websocket.svelte';
	import {
		getServers,
		createServer,
		getInviteInfo,
		joinByCode,
		joinByName,
		type Server,
		type InviteInfo
	} from '$lib/api';

	let { children } = $props();

	let servers = $state<Server[]>([]);
	let loading = $state(true);
	let showCreateModal = $state(false);
	let newServerName = $state('');
	let creating = $state(false);

	// Join server state
	let showJoinModal = $state(false);
	let joinTab = $state<'code' | 'name'>('code');
	let inviteCode = $state('');
	let joinServerName = $state('');
	let joinPassword = $state('');
	let joining = $state(false);
	let joinError = $state('');
	let invitePreview = $state<InviteInfo | null>(null);
	let loadingPreview = $state(false);

	onMount(async () => {
		await auth.init();
		if (!auth.isAuthenticated) {
			goto('/');
			return;
		}
		await loadServers();

		// Connect WebSocket for real-time updates
		websocket.connect();

		return () => {
			// Disconnect WebSocket when leaving app
			websocket.disconnect();
		};
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

	function resetJoinModal() {
		inviteCode = '';
		joinServerName = '';
		joinPassword = '';
		joinError = '';
		invitePreview = null;
		joinTab = 'code';
	}

	async function handlePreviewInvite() {
		if (!inviteCode.trim()) return;

		loadingPreview = true;
		joinError = '';
		const result = await getInviteInfo(inviteCode.trim());
		loadingPreview = false;

		if (result.error) {
			joinError = result.error;
			invitePreview = null;
		} else if (result.data) {
			invitePreview = result.data;
		}
	}

	async function handleJoinByCode(e: Event) {
		e.preventDefault();
		if (!inviteCode.trim()) return;

		joining = true;
		joinError = '';
		const result = await joinByCode(inviteCode.trim());
		joining = false;

		if (result.error) {
			joinError = result.error;
		} else if (result.data) {
			servers = [...servers, result.data];
			showJoinModal = false;
			resetJoinModal();
			goto(`/app/${result.data.id}`);
		}
	}

	async function handleJoinByName(e: Event) {
		e.preventDefault();
		if (!joinServerName.trim() || !joinPassword) return;

		joining = true;
		joinError = '';
		const result = await joinByName(joinServerName.trim(), joinPassword);
		joining = false;

		if (result.error) {
			joinError = result.error;
		} else if (result.data) {
			servers = [...servers, result.data];
			showJoinModal = false;
			resetJoinModal();
			goto(`/app/${result.data.id}`);
		}
	}
</script>

<div class="app-layout">
	<aside class="server-bar">
		<a href="/app/dms" class="server-icon dm-icon" title="Direct Messages">
			DM
		</a>
		<div class="server-divider"></div>

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

		<button class="server-icon join-server" title="Join Server" onclick={() => { resetJoinModal(); showJoinModal = true; }}>
			<span>→</span>
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

{#if showJoinModal}
	<div class="modal-backdrop" onclick={() => (showJoinModal = false)}>
		<div class="modal" onclick={(e) => e.stopPropagation()}>
			<h2>Join a Server</h2>

			<div class="tab-bar">
				<button
					class="tab"
					class:active={joinTab === 'code'}
					onclick={() => { joinTab = 'code'; joinError = ''; }}
				>
					Invite Code
				</button>
				<button
					class="tab"
					class:active={joinTab === 'name'}
					onclick={() => { joinTab = 'name'; joinError = ''; }}
				>
					Server Name
				</button>
			</div>

			{#if joinError}
				<div class="error-message">{joinError}</div>
			{/if}

			{#if joinTab === 'code'}
				<form onsubmit={handleJoinByCode}>
					<div class="form-group">
						<label for="invite-code">Invite Code</label>
						<div class="input-with-button">
							<input
								type="text"
								id="invite-code"
								bind:value={inviteCode}
								placeholder="e.g. AbC12xYz"
								required
							/>
							<button
								type="button"
								class="secondary small"
								onclick={handlePreviewInvite}
								disabled={loadingPreview || !inviteCode.trim()}
							>
								{loadingPreview ? '...' : 'Preview'}
							</button>
						</div>
					</div>

					{#if invitePreview}
						<div class="invite-preview">
							<div class="preview-server-name">{invitePreview.server_name}</div>
							<div class="preview-member-count">{invitePreview.member_count} members</div>
						</div>
					{/if}

					<div class="modal-actions">
						<button type="button" class="secondary" onclick={() => (showJoinModal = false)}>
							Cancel
						</button>
						<button type="submit" class="primary" disabled={joining || !inviteCode.trim()}>
							{joining ? 'Joining...' : 'Join Server'}
						</button>
					</div>
				</form>
			{:else}
				<form onsubmit={handleJoinByName}>
					<div class="form-group">
						<label for="join-server-name">Server Name</label>
						<input
							type="text"
							id="join-server-name"
							bind:value={joinServerName}
							placeholder="Enter exact server name"
							required
						/>
					</div>
					<div class="form-group">
						<label for="join-password">Password</label>
						<input
							type="password"
							id="join-password"
							bind:value={joinPassword}
							placeholder="Enter server password"
							required
						/>
					</div>
					<div class="modal-actions">
						<button type="button" class="secondary" onclick={() => (showJoinModal = false)}>
							Cancel
						</button>
						<button type="submit" class="primary" disabled={joining || !joinServerName.trim() || !joinPassword}>
							{joining ? 'Joining...' : 'Join Server'}
						</button>
					</div>
				</form>
			{/if}
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

	.server-icon.dm-icon {
		background-color: var(--accent);
		font-size: 14px;
		font-weight: 700;
	}

	.server-divider {
		width: 32px;
		height: 2px;
		background-color: var(--border);
		border-radius: 1px;
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

	.server-icon.join-server {
		background-color: transparent;
		border: 2px dashed var(--border);
		color: var(--success, #3ba55c);
		font-size: 20px;
	}

	.server-icon.join-server span {
		transform: translateY(-2px);
	}

	.server-icon.join-server:hover {
		border-color: var(--success, #3ba55c);
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

	/* Tab bar styles */
	.tab-bar {
		display: flex;
		gap: 4px;
		margin-bottom: 16px;
		border-bottom: 2px solid var(--border);
	}

	.tab {
		padding: 8px 16px;
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 14px;
		border-bottom: 2px solid transparent;
		margin-bottom: -2px;
		transition: color 0.2s, border-color 0.2s;
	}

	.tab:hover {
		color: var(--text-primary);
	}

	.tab.active {
		color: var(--accent);
		border-bottom-color: var(--accent);
	}

	/* Error message */
	.error-message {
		background: rgba(237, 66, 69, 0.1);
		border: 1px solid var(--danger);
		color: var(--danger);
		padding: 10px 12px;
		border-radius: 4px;
		margin-bottom: 16px;
		font-size: 14px;
	}

	/* Input with button */
	.input-with-button {
		display: flex;
		gap: 8px;
	}

	.input-with-button input {
		flex: 1;
	}

	.input-with-button button.small {
		padding: 8px 12px;
		font-size: 12px;
		white-space: nowrap;
	}

	/* Invite preview */
	.invite-preview {
		background: var(--bg-primary);
		border-radius: 8px;
		padding: 16px;
		margin-bottom: 16px;
		text-align: center;
	}

	.preview-server-name {
		font-size: 18px;
		font-weight: 600;
		color: var(--text-primary);
		margin-bottom: 4px;
	}

	.preview-member-count {
		font-size: 12px;
		color: var(--text-muted);
	}
</style>
