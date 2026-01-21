<script lang="ts">
	import { onMount, setContext } from 'svelte';
	import {
		getConversations,
		searchUsers,
		startConversation,
		type Conversation,
		type UserSearchResult
	} from '$lib/api';
	import { goto } from '$app/navigation';

	let { children } = $props();

	let conversations = $state<Conversation[]>([]);
	let loading = $state(true);

	// New DM modal state
	let showNewDm = $state(false);
	let searchQuery = $state('');
	let searchResults = $state<UserSearchResult[]>([]);
	let searching = $state(false);
	let startingConversation = $state(false);

	// Share conversations with children
	const conversationsContext = $state<{ conversations: Conversation[] }>({ conversations: [] });
	setContext('conversations', conversationsContext);

	onMount(() => {
		loadConversations();
	});

	async function loadConversations() {
		loading = true;
		const result = await getConversations();
		if (result.data) {
			conversations = result.data;
			conversationsContext.conversations = result.data;
		}
		loading = false;
	}

	async function handleSearch() {
		if (!searchQuery.trim() || searchQuery.length < 2) {
			searchResults = [];
			return;
		}

		searching = true;
		const result = await searchUsers(searchQuery.trim());
		searching = false;

		if (result.data) {
			searchResults = result.data;
		}
	}

	async function handleStartConversation(userId: string) {
		startingConversation = true;
		const result = await startConversation(userId);
		startingConversation = false;

		if (result.data) {
			// Add to list if not already there
			const exists = conversations.find((c) => c.id === result.data!.id);
			if (!exists) {
				conversations = [result.data, ...conversations];
				conversationsContext.conversations = conversations;
			}
			showNewDm = false;
			searchQuery = '';
			searchResults = [];
			goto(`/app/dms/${result.data.id}`);
		}
	}

	function formatTime(timestamp: number): string {
		const date = new Date(timestamp);
		const now = new Date();
		const diffMs = now.getTime() - date.getTime();
		const diffDays = Math.floor(diffMs / (1000 * 60 * 60 * 24));

		if (diffDays === 0) {
			return date.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
		} else if (diffDays === 1) {
			return 'Yesterday';
		} else if (diffDays < 7) {
			return date.toLocaleDateString([], { weekday: 'short' });
		} else {
			return date.toLocaleDateString([], { month: 'short', day: 'numeric' });
		}
	}
</script>

<div class="dms-layout">
	<aside class="dms-sidebar">
		<div class="sidebar-header">
			<h2>Direct Messages</h2>
			<button class="new-dm-btn" title="New Message" onclick={() => (showNewDm = true)}>
				+
			</button>
		</div>

		<div class="conversations-list">
			{#if loading}
				<p class="loading">Loading...</p>
			{:else if conversations.length === 0}
				<p class="empty">No conversations yet. Start one by clicking + above.</p>
			{:else}
				{#each conversations as convo}
					<a href="/app/dms/{convo.id}" class="conversation-item">
						<div class="avatar">
							{convo.other_username.charAt(0).toUpperCase()}
						</div>
						<div class="conversation-info">
							<div class="conversation-header">
								<span class="username">{convo.other_username}</span>
								<span class="time">{formatTime(convo.updated_at)}</span>
							</div>
							{#if convo.last_message_preview}
								<p class="preview">{convo.last_message_preview}</p>
							{/if}
						</div>
					</a>
				{/each}
			{/if}
		</div>
	</aside>

	<div class="dms-content">
		{@render children()}
	</div>
</div>

{#if showNewDm}
	<div class="modal-backdrop" onclick={() => (showNewDm = false)}>
		<div class="modal" onclick={(e) => e.stopPropagation()}>
			<h2>New Message</h2>
			<div class="form-group">
				<label for="user-search">Search for a user</label>
				<div class="search-input">
					<input
						type="text"
						id="user-search"
						bind:value={searchQuery}
						oninput={handleSearch}
						placeholder="Enter username..."
					/>
					{#if searching}
						<span class="searching">...</span>
					{/if}
				</div>
			</div>

			<div class="search-results">
				{#if searchResults.length > 0}
					{#each searchResults as user}
						<button
							class="user-result"
							onclick={() => handleStartConversation(user.id)}
							disabled={startingConversation}
						>
							<div class="result-avatar">
								{user.username.charAt(0).toUpperCase()}
							</div>
							<span class="result-username">{user.username}</span>
						</button>
					{/each}
				{:else if searchQuery.length >= 2 && !searching}
					<p class="no-results">No users found</p>
				{:else if searchQuery.length > 0}
					<p class="hint">Type at least 2 characters to search</p>
				{/if}
			</div>

			<div class="modal-actions">
				<button class="secondary" onclick={() => (showNewDm = false)}>Cancel</button>
			</div>
		</div>
	</div>
{/if}

<style>
	.dms-layout {
		display: flex;
		flex: 1;
		overflow: hidden;
	}

	.dms-sidebar {
		width: 240px;
		background-color: var(--bg-secondary);
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.sidebar-header {
		padding: 16px;
		border-bottom: 1px solid var(--bg-tertiary);
		display: flex;
		align-items: center;
		justify-content: space-between;
	}

	.sidebar-header h2 {
		font-size: 16px;
		font-weight: 600;
		margin: 0;
	}

	.new-dm-btn {
		width: 24px;
		height: 24px;
		border-radius: 4px;
		background: none;
		border: none;
		color: var(--text-muted);
		font-size: 18px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.new-dm-btn:hover {
		background-color: var(--bg-primary);
		color: var(--text-primary);
	}

	.conversations-list {
		flex: 1;
		overflow-y: auto;
		padding: 8px;
	}

	.loading,
	.empty {
		font-size: 13px;
		color: var(--text-muted);
		padding: 16px;
		text-align: center;
	}

	.conversation-item {
		display: flex;
		align-items: center;
		gap: 12px;
		padding: 10px 12px;
		border-radius: 6px;
		text-decoration: none;
		color: var(--text-primary);
		transition: background-color 0.15s;
	}

	.conversation-item:hover {
		background-color: var(--bg-primary);
	}

	.avatar {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		background-color: var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 16px;
		flex-shrink: 0;
	}

	.conversation-info {
		flex: 1;
		min-width: 0;
	}

	.conversation-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		gap: 8px;
	}

	.username {
		font-weight: 500;
		font-size: 14px;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.time {
		font-size: 11px;
		color: var(--text-muted);
		flex-shrink: 0;
	}

	.preview {
		font-size: 13px;
		color: var(--text-muted);
		margin: 2px 0 0;
		white-space: nowrap;
		overflow: hidden;
		text-overflow: ellipsis;
	}

	.dms-content {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
		background-color: var(--bg-primary);
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

	.search-input {
		position: relative;
	}

	.search-input input {
		width: 100%;
	}

	.searching {
		position: absolute;
		right: 12px;
		top: 50%;
		transform: translateY(-50%);
		color: var(--text-muted);
	}

	.search-results {
		max-height: 200px;
		overflow-y: auto;
		margin-bottom: 16px;
	}

	.user-result {
		display: flex;
		align-items: center;
		gap: 12px;
		width: 100%;
		padding: 10px 12px;
		border: none;
		background: none;
		border-radius: 6px;
		cursor: pointer;
		text-align: left;
		color: var(--text-primary);
	}

	.user-result:hover {
		background-color: var(--bg-primary);
	}

	.user-result:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.result-avatar {
		width: 32px;
		height: 32px;
		border-radius: 50%;
		background-color: var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 14px;
	}

	.result-username {
		font-size: 14px;
	}

	.no-results,
	.hint {
		font-size: 13px;
		color: var(--text-muted);
		padding: 12px;
		text-align: center;
	}

	.modal-actions {
		display: flex;
		justify-content: flex-end;
		gap: 12px;
	}
</style>
