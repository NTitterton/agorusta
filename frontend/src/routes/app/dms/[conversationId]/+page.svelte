<script lang="ts">
	import { page } from '$app/stores';
	import { getContext, onMount, onDestroy } from 'svelte';
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

	// Typing indicator state
	let typingUsers = $state<Map<string, string>>(new Map());
	let typingTimeout: ReturnType<typeof setTimeout> | null = null;
	let isTyping = false;

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

	// Subscribe to typing indicators
	$effect(() => {
		if (!conversationId) return;

		const unsubscribe = websocket.subscribeToTyping(conversationId, (users) => {
			typingUsers = users;
		});

		return () => {
			unsubscribe();
			// Clear typing state when leaving conversation
			if (isTyping) {
				websocket.sendStopTyping(conversationId);
				isTyping = false;
			}
			if (typingTimeout) {
				clearTimeout(typingTimeout);
				typingTimeout = null;
			}
		};
	});

	function handleTyping() {
		if (!conversationId) return;

		// Send typing indicator
		if (!isTyping) {
			websocket.sendTyping(conversationId);
			isTyping = true;
		}

		// Reset the stop-typing timeout
		if (typingTimeout) {
			clearTimeout(typingTimeout);
		}
		typingTimeout = setTimeout(() => {
			if (isTyping && conversationId) {
				websocket.sendStopTyping(conversationId);
				isTyping = false;
			}
		}, 3000);
	}

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

		// Clear typing state
		if (isTyping && conversationId) {
			websocket.sendStopTyping(conversationId);
			isTyping = false;
		}
		if (typingTimeout) {
			clearTimeout(typingTimeout);
			typingTimeout = null;
		}

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

	// Parse message content for markdown links and images
	function parseMessageContent(content: string): { type: 'text' | 'link' | 'image'; text: string; url?: string }[] {
		const parts: { type: 'text' | 'link' | 'image'; text: string; url?: string }[] = [];
		const linkRegex = /\[([^\]]+)\]\(([^)]+)\)/g;
		let lastIndex = 0;
		let match;

		while ((match = linkRegex.exec(content)) !== null) {
			// Add text before the link
			if (match.index > lastIndex) {
				parts.push({ type: 'text', text: content.slice(lastIndex, match.index) });
			}

			const [, linkText, url] = match;
			const isImage = /\.(jpg|jpeg|png|gif|webp|svg)(\?|$)/i.test(url);

			parts.push({
				type: isImage ? 'image' : 'link',
				text: linkText,
				url
			});

			lastIndex = match.index + match[0].length;
		}

		// Add remaining text
		if (lastIndex < content.length) {
			parts.push({ type: 'text', text: content.slice(lastIndex) });
		}

		return parts.length > 0 ? parts : [{ type: 'text', text: content }];
	}

	// Typing indicator text - for DMs it's simpler since there's only one other person
	let typingText = $derived.by(() => {
		const users = Array.from(typingUsers.values());
		if (users.length === 0) return '';
		return `${conversation?.other_username ?? 'User'} is typing...`;
	});
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
							<div class="message-content">
								{#each parseMessageContent(message.content) as part}
									{#if part.type === 'text'}
										{part.text}
									{:else if part.type === 'image'}
										<a href={part.url} target="_blank" rel="noopener noreferrer" class="message-image-link">
											<img src={part.url} alt={part.text} class="message-image" />
										</a>
									{:else}
										<a href={part.url} target="_blank" rel="noopener noreferrer" class="message-link">{part.text}</a>
									{/if}
								{/each}
							</div>
						</div>
					</div>
				{/each}
			{/each}
		{/if}
	</div>

	<div class="message-input-container">
		{#if typingText}
			<div class="typing-indicator">
				<span class="typing-dots"></span>
				{typingText}
			</div>
		{/if}
		<form onsubmit={handleSend}>
			<input
				type="text"
				class="message-input"
				placeholder="Message @{conversation?.other_username ?? 'user'}"
				bind:value={messageInput}
				oninput={handleTyping}
				disabled={sending}
			/>
		</form>
	</div>
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
		white-space: pre-wrap;
	}

	.message-link {
		color: var(--accent);
		text-decoration: none;
	}

	.message-link:hover {
		text-decoration: underline;
	}

	.message-image-link {
		display: block;
		margin-top: 8px;
	}

	.message-image {
		max-width: 400px;
		max-height: 300px;
		border-radius: 8px;
		cursor: pointer;
	}

	.message-image:hover {
		opacity: 0.9;
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

	.typing-indicator {
		font-size: 12px;
		color: var(--text-muted);
		padding: 0 4px 6px;
		display: flex;
		align-items: center;
		gap: 6px;
	}

	.typing-dots {
		display: inline-flex;
		gap: 2px;
	}

	.typing-dots::before,
	.typing-dots::after {
		content: '';
		width: 4px;
		height: 4px;
		border-radius: 50%;
		background: var(--text-muted);
		animation: typing-bounce 1.4s infinite ease-in-out;
	}

	.typing-dots::before {
		animation-delay: 0s;
	}

	.typing-dots::after {
		animation-delay: 0.2s;
	}

	.typing-dots {
		position: relative;
	}

	.typing-dots::before {
		box-shadow: 6px 0 0 var(--text-muted);
	}

	@keyframes typing-bounce {
		0%, 60%, 100% {
			transform: translateY(0);
		}
		30% {
			transform: translateY(-3px);
		}
	}
</style>
