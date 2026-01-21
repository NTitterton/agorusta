<script lang="ts">
	import { page } from '$app/stores';
	import { onMount, setContext } from 'svelte';
	import { getServer, createChannel, type ServerWithChannels, type Channel } from '$lib/api';

	let { children } = $props();

	// Share server data with child components
	const serverContext = $state<{ server: ServerWithChannels | null }>({ server: null });
	setContext('server', serverContext);

	let server = $state<ServerWithChannels | null>(null);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let showCreateChannel = $state(false);
	let newChannelName = $state('');
	let creating = $state(false);

	$effect(() => {
		const serverId = $page.params.serverId;
		if (serverId) {
			loadServer(serverId);
		}
	});

	async function loadServer(serverId: string) {
		loading = true;
		error = null;
		const result = await getServer(serverId);
		if (result.data) {
			server = result.data;
			serverContext.server = result.data;
		} else {
			error = result.error || 'Failed to load server';
			serverContext.server = null;
		}
		loading = false;
	}

	async function handleCreateChannel(e: Event) {
		e.preventDefault();
		if (!newChannelName.trim() || !server) return;

		creating = true;
		const result = await createChannel(server.id, newChannelName.trim());
		creating = false;

		if (result.data && server) {
			const updatedServer = {
				...server,
				channels: [...server.channels, result.data]
			};
			server = updatedServer;
			serverContext.server = updatedServer;
			newChannelName = '';
			showCreateChannel = false;
		}
	}
</script>

<div class="server-layout">
	<aside class="channel-bar">
		{#if loading}
			<div class="channel-header">
				<span class="loading-text">Loading...</span>
			</div>
		{:else if error}
			<div class="channel-header">
				<span class="error-text">{error}</span>
			</div>
		{:else if server}
			<div class="channel-header">
				<h2>{server.name}</h2>
			</div>

			<div class="channel-category">
				<div class="category-header">
					<span>Text Channels</span>
					<button class="add-channel" title="Create Channel" onclick={() => (showCreateChannel = true)}>
						+
					</button>
				</div>

				{#each server.channels.filter(c => c.channel_type === 'text') as channel}
					<a href="/app/{server.id}/{channel.id}" class="channel-link">
						<span class="channel-hash">#</span>
						{channel.name}
					</a>
				{/each}

				{#if server.channels.filter(c => c.channel_type === 'text').length === 0}
					<p class="no-channels">No channels yet</p>
				{/if}
			</div>

			{#if showCreateChannel}
				<div class="create-channel-form">
					<form onsubmit={handleCreateChannel}>
						<input
							type="text"
							bind:value={newChannelName}
							placeholder="new-channel"
							required
						/>
						<div class="form-actions">
							<button type="button" class="secondary small" onclick={() => (showCreateChannel = false)}>
								Cancel
							</button>
							<button type="submit" class="primary small" disabled={creating}>
								{creating ? '...' : 'Create'}
							</button>
						</div>
					</form>
				</div>
			{/if}

			<div class="channel-bar-footer">
				<span class="member-count">{server.member_count} member{server.member_count !== 1 ? 's' : ''}</span>
			</div>
		{/if}
	</aside>

	<div class="server-content">
		{@render children()}
	</div>
</div>

<style>
	.server-layout {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.channel-bar {
		width: 240px;
		background-color: var(--bg-secondary);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.channel-header {
		padding: 16px;
		border-bottom: 1px solid var(--bg-tertiary);
		min-height: 52px;
		display: flex;
		align-items: center;
	}

	.channel-header h2 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.loading-text,
	.error-text {
		font-size: 14px;
		color: var(--text-muted);
	}

	.error-text {
		color: var(--danger);
	}

	.channel-category {
		padding: 16px 8px;
		flex: 1;
		overflow-y: auto;
	}

	.category-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		padding: 0 8px;
		margin-bottom: 4px;
	}

	.category-header span {
		font-size: 12px;
		text-transform: uppercase;
		color: var(--text-muted);
		font-weight: 600;
		letter-spacing: 0.5px;
	}

	.add-channel {
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: 16px;
		cursor: pointer;
		padding: 0;
		width: 18px;
		height: 18px;
		display: flex;
		align-items: center;
		justify-content: center;
		line-height: 1;
	}

	.add-channel:hover {
		color: var(--text-primary);
	}

	.channel-link {
		display: flex;
		align-items: center;
		padding: 8px 12px;
		border-radius: 4px;
		color: var(--text-muted);
		text-decoration: none;
		font-size: 14px;
		gap: 6px;
	}

	.channel-link:hover {
		background-color: var(--bg-primary);
		color: var(--text-primary);
	}

	.channel-hash {
		color: var(--text-muted);
		font-weight: 500;
	}

	.no-channels {
		font-size: 12px;
		color: var(--text-muted);
		padding: 8px 12px;
		font-style: italic;
	}

	.create-channel-form {
		padding: 8px;
		border-top: 1px solid var(--bg-tertiary);
	}

	.create-channel-form input {
		width: 100%;
		margin-bottom: 8px;
		font-size: 14px;
		padding: 8px;
	}

	.form-actions {
		display: flex;
		gap: 8px;
		justify-content: flex-end;
	}

	button.small {
		padding: 6px 12px;
		font-size: 12px;
	}

	.channel-bar-footer {
		padding: 12px 16px;
		border-top: 1px solid var(--bg-tertiary);
	}

	.member-count {
		font-size: 12px;
		color: var(--text-muted);
	}

	.server-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background-color: var(--bg-primary);
	}
</style>
