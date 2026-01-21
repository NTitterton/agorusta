<script lang="ts">
	import { page } from '$app/stores';
	import { getContext, onMount } from 'svelte';
	import {
		getConversation,
		getDmMessages,
		sendDmMessage,
		type Conversation,
		type DirectMessage
	} from '$lib/api';
	import { websocket } from '$lib/websocket.svelte';

	let conversationId = $derived($page.params.conversationId);

	let conversation = $state<Conversation | null>(null);
	let messages = $state<DirectMessage[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);
	let hasMore = $state(false);
	let loadingMore = $state(false);

	let messageInput = $state('');
	let sending = $state(false);

	let messagesContainer: HTMLElement;

	// Load conversation and messages when ID changes
	$effect(() => {
		if (conversationId) {
			loadConversation();
			loadMessages();
		}
	});

	// Subscribe to real-time updates
	$effect(() => {
		if (!conversationId) return;

		const unsubscribe = websocket.subscribeToConversation(conversationId, (newMessage) => {
			// Check if message already exists (deduplication)
			if (!messages.find((m) => m.id === newMessage.id)) {
				messages = [...messages, newMessage];
				scrollToBottom();
			}
		});

		return unsubscribe;
	});

	async function loadConversation() {
		const result = await getConversation(conversationId);
		if (result.data) {
			conversation = result.data;
		}
	}

	async function loadMessages() {
		loading = true;
		error = null;
		messages = [];

		const result = await getDmMessages(conversationId, { limit: 50 });

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

		const result = await getDmMessages(conversationId, {
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

		const result = await sendDmMessage(conversationId, content);

		if (result.error) {
			// Restore input on error
			messageInput = content;
			error = result.error;
		} else if (result.data) {
			// Add message immediately (deduplication will handle if WS also delivers it)
			if (!messages.find((m) => m.id === result.data!.id)) {
				messages = [...messages, result.data];
				scrollToBottom();
			}
		}

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
	function getMessageGroups(msgs: DirectMessage[]): { date: string; messages: DirectMessage[] }[] {
		const groups: { date: string; messages: DirectMessage[] }[] = [];
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

<div class="dm-view">
	<header class="dm-header">
		<div class="header-avatar">
			{conversation?.other_username?.charAt(0).toUpperCase() ?? '?'}
		</div>
		<span class="header-username">{conversation?.other_username ?? 'Loading...'}</span>
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
				<div class="empty-avatar">
					{conversation?.other_username?.charAt(0).toUpperCase() ?? '?'}
				</div>
				<h3>{conversation?.other_username}</h3>
				<p>This is the beginning of your direct message history with {conversation?.other_username}.</p>
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
			placeholder="Message @{conversation?.other_username ?? 'user'}"
			bind:value={messageInput}
			disabled={sending}
		/>
	</form>
</div>

<style>
	.dm-view {
		flex: 1;
		display: flex;
		flex-direction: column;
		overflow: hidden;
	}

	.dm-header {
		padding: 12px 16px;
		border-bottom: 1px solid var(--bg-tertiary);
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.header-avatar {
		width: 28px;
		height: 28px;
		border-radius: 50%;
		background-color: var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 12px;
	}

	.header-username {
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
	.error-state {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		color: var(--text-muted);
		padding: 16px;
	}

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

	.empty-avatar {
		width: 80px;
		height: 80px;
		border-radius: 50%;
		background-color: var(--accent);
		display: flex;
		align-items: center;
		justify-content: center;
		font-weight: 600;
		font-size: 32px;
		margin-bottom: 16px;
	}

	.empty-messages h3 {
		color: var(--text-primary);
		margin-bottom: 8px;
		font-size: 20px;
	}

	.empty-messages p {
		max-width: 400px;
		line-height: 1.4;
	}

	.error-state button {
		margin-top: 12px;
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
