<script lang="ts">
	import { page } from '$app/stores';
	import { getContext, onMount } from 'svelte';
	import { getMessages, sendMessage, type Message, type ServerWithChannels } from '$lib/api';
	import { websocket } from '$lib/websocket.svelte';

	const serverContext = getContext<{ server: ServerWithChannels | null }>('server');

	let serverId = $derived($page.params.serverId);
	let channelId = $derived($page.params.channelId);
	let channel = $derived(serverContext.server?.channels.find((c) => c.id === channelId));

	let messages = $state<Message[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let hasMore = $state(false);
	let loadingMore = $state(false);

	let messageInput = $state('');
	let sending = $state(false);

	let messagesContainer: HTMLElement;

	// Load messages when channel changes
	$effect(() => {
		if (channelId && serverId) {
			loadMessages();
		}
	});

	// Subscribe to real-time updates
	$effect(() => {
		if (!channelId) return;

		const unsubscribe = websocket.subscribeToChannel(channelId, (newMessage) => {
			// Check if message already exists (deduplication)
			if (!messages.find((m) => m.id === newMessage.id)) {
				messages = [...messages, newMessage];
				scrollToBottom();
			}
		});

		return unsubscribe;
	});

	async function loadMessages() {
		loading = true;
		error = null;
		messages = [];

		const result = await getMessages(serverId, channelId, { limit: 50 });

		if (result.data) {
			// Messages come newest-first, reverse for display (oldest at top)
			messages = result.data.messages.reverse();
			hasMore = result.data.has_more;
			scrollToBottom();
		} else {
			error = result.error || 'Failed to load messages';
		}

		loading = false;
	}

	async function loadMoreMessages() {
		if (loadingMore || !hasMore || messages.length === 0) return;

		loadingMore = true;
		const oldestMessage = messages[0];

		const result = await getMessages(serverId, channelId, {
			limit: 50,
			before: oldestMessage.created_at
		});

		if (result.data) {
			// Prepend older messages
			messages = [...result.data.messages.reverse(), ...messages];
			hasMore = result.data.has_more;
		}

		loadingMore = false;
	}

	async function handleSend(e: Event) {
		e.preventDefault();
		if (!messageInput.trim() || sending) return;

		sending = true;
		const content = messageInput.trim();
		messageInput = '';

		const result = await sendMessage(serverId, channelId, content);

		if (result.error) {
			// Restore input on error
			messageInput = content;
			error = result.error;
		}
		// Note: Message will arrive via WebSocket broadcast

		sending = false;
	}

	function scrollToBottom() {
		requestAnimationFrame(() => {
			if (messagesContainer) {
				messagesContainer.scrollTop = messagesContainer.scrollHeight;
			}
		});
	}

	function formatTime(timestamp: number): string {
		return new Date(timestamp).toLocaleTimeString([], {
			hour: '2-digit',
			minute: '2-digit'
		});
	}

	function formatDate(timestamp: number): string {
		const date = new Date(timestamp);
		const today = new Date();
		const yesterday = new Date(today);
		yesterday.setDate(yesterday.getDate() - 1);

		if (date.toDateString() === today.toDateString()) {
			return 'Today';
		} else if (date.toDateString() === yesterday.toDateString()) {
			return 'Yesterday';
		}
		return date.toLocaleDateString([], { month: 'short', day: 'numeric', year: 'numeric' });
	}

	// Group messages by date
	function getMessageGroups(msgs: Message[]): { date: string; messages: Message[] }[] {
		const groups: { date: string; messages: Message[] }[] = [];
		let currentDate = '';

		for (const msg of msgs) {
			const date = formatDate(msg.created_at);
			if (date !== currentDate) {
				currentDate = date;
				groups.push({ date, messages: [] });
			}
			groups[groups.length - 1].messages.push(msg);
		}

		return groups;
	}

	let messageGroups = $derived(getMessageGroups(messages));
</script>

<div class="channel-view">
	<header class="channel-header">
		<span class="channel-hash">#</span>
		<span class="channel-name">{channel?.name ?? 'Loading...'}</span>
	</header>

	<div class="messages-container" bind:this={messagesContainer}>
		{#if loading}
			<div class="loading-state">
				<p>Loading messages...</p>
			</div>
		{:else if error}
			<div class="error-state">
				<p>{error}</p>
				<button onclick={loadMessages}>Retry</button>
			</div>
		{:else if messages.length === 0}
			<div class="empty-messages">
				<h3>Welcome to #{channel?.name}!</h3>
				<p>This is the beginning of the channel.</p>
			</div>
		{:else}
			{#if hasMore}
				<button class="load-more" onclick={loadMoreMessages} disabled={loadingMore}>
					{loadingMore ? 'Loading...' : 'Load older messages'}
				</button>
			{/if}

			{#each messageGroups as group}
				<div class="date-divider">
					<span>{group.date}</span>
				</div>

				{#each group.messages as message (message.id)}
					<div class="message">
						<div class="message-avatar">
							{message.author_username.charAt(0).toUpperCase()}
						</div>
						<div class="message-body">
							<div class="message-header">
								<span class="message-author">{message.author_username}</span>
								<span class="message-time">{formatTime(message.created_at)}</span>
							</div>
							<div class="message-content">{message.content}</div>
						</div>
					</div>
				{/each}
			{/each}
		{/if}
	</div>

	<form class="message-input-container" onsubmit={handleSend}>
		<input
			type="text"
			class="message-input"
			placeholder="Message #{channel?.name ?? 'channel'}"
			bind:value={messageInput}
			disabled={sending}
		/>
	</form>
</div>

<style>
	.channel-view {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.channel-header {
		padding: 12px 16px;
		border-bottom: 1px solid var(--bg-tertiary);
		display: flex;
		align-items: center;
		gap: 4px;
	}

	.channel-hash {
		color: var(--text-muted);
		font-size: 20px;
		font-weight: 500;
	}

	.channel-name {
		font-weight: 600;
		font-size: 16px;
	}

	.messages-container {
		flex: 1;
		overflow-y: auto;
		padding: 16px 0;
		display: flex;
		flex-direction: column;
	}

	.loading-state,
	.error-state,
	.empty-messages {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		color: var(--text-muted);
		padding: 16px;
	}

	.error-state button {
		margin-top: 12px;
	}

	.empty-messages h3 {
		color: var(--text-primary);
		margin-bottom: 8px;
	}

	.load-more {
		width: fit-content;
		margin: 0 auto 16px;
		padding: 8px 16px;
		background: transparent;
		border: 1px solid var(--border);
		border-radius: 4px;
		color: var(--text-muted);
		cursor: pointer;
		font-size: 13px;
	}

	.load-more:hover:not(:disabled) {
		color: var(--text-primary);
		border-color: var(--text-muted);
	}

	.load-more:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.date-divider {
		display: flex;
		align-items: center;
		margin: 16px 16px 8px;
	}

	.date-divider::before,
	.date-divider::after {
		content: '';
		flex: 1;
		height: 1px;
		background: var(--bg-tertiary);
	}

	.date-divider span {
		padding: 0 16px;
		font-size: 12px;
		color: var(--text-muted);
		font-weight: 600;
	}

	.message {
		display: flex;
		gap: 12px;
		padding: 4px 16px;
		margin-top: 16px;
	}

	.message:hover {
		background-color: var(--bg-secondary);
	}

	.message-avatar {
		width: 40px;
		height: 40px;
		border-radius: 50%;
		background-color: var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 16px;
		color: var(--bg-primary);
		flex-shrink: 0;
	}

	.message-body {
		flex: 1;
		min-width: 0;
	}

	.message-header {
		display: flex;
		align-items: baseline;
		gap: 8px;
	}

	.message-author {
		font-weight: 600;
		color: var(--text-primary);
	}

	.message-time {
		font-size: 12px;
		color: var(--text-muted);
	}

	.message-content {
		color: var(--text-secondary);
		line-height: 1.4;
		word-wrap: break-word;
		margin-top: 4px;
	}

	.message-input-container {
		padding: 16px;
	}

	.message-input {
		width: 100%;
		padding: 12px 16px;
		border-radius: 8px;
		font-size: 14px;
	}

	.message-input:disabled {
		opacity: 0.6;
	}
</style>
