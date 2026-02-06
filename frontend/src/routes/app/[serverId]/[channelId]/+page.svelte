<script lang="ts">
	import { page } from '$app/stores';
	import { getContext, onMount, onDestroy } from 'svelte';
	import { getMessages, sendMessage, uploadFile, type Message, type ServerWithChannels, type Attachment } from '$lib/api';
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
	let fileInput: HTMLInputElement;

	// Typing indicator state
	let typingUsers = $state<Map<string, string>>(new Map());
	let typingTimeout: ReturnType<typeof setTimeout> | null = null;
	let isTyping = false;

	// File upload state
	let pendingFiles = $state<File[]>([]);
	let uploading = $state(false);
	let uploadError = $state<string | null>(null);

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

	// Subscribe to typing indicators
	$effect(() => {
		if (!channelId) return;

		const unsubscribe = websocket.subscribeToTyping(channelId, (users) => {
			typingUsers = users;
		});

		return () => {
			unsubscribe();
			// Clear typing state when leaving channel
			if (isTyping) {
				websocket.sendStopTyping(channelId);
				isTyping = false;
			}
			if (typingTimeout) {
				clearTimeout(typingTimeout);
				typingTimeout = null;
			}
		};
	});

	function handleTyping() {
		if (!channelId) return;

		// Send typing indicator
		if (!isTyping) {
			websocket.sendTyping(channelId);
			isTyping = true;
		}

		// Reset the stop-typing timeout
		if (typingTimeout) {
			clearTimeout(typingTimeout);
		}
		typingTimeout = setTimeout(() => {
			if (isTyping && channelId) {
				websocket.sendStopTyping(channelId);
				isTyping = false;
			}
		}, 3000);
	}

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

	function handleFileSelect(e: Event) {
		const input = e.target as HTMLInputElement;
		if (input.files) {
			pendingFiles = [...pendingFiles, ...Array.from(input.files)];
			input.value = ''; // Reset input
		}
	}

	function removeFile(index: number) {
		pendingFiles = pendingFiles.filter((_, i) => i !== index);
	}

	function formatFileSize(bytes: number): string {
		if (bytes < 1024) return bytes + ' B';
		if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
		return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
	}

	async function handleSend(e: Event) {
		e.preventDefault();
		if ((!messageInput.trim() && pendingFiles.length === 0) || sending) return;

		// Clear typing state
		if (isTyping && channelId) {
			websocket.sendStopTyping(channelId);
			isTyping = false;
		}
		if (typingTimeout) {
			clearTimeout(typingTimeout);
			typingTimeout = null;
		}

		sending = true;
		uploading = pendingFiles.length > 0;
		uploadError = null;

		let content = messageInput.trim();
		messageInput = '';

		// Upload files and append URLs to content
		if (pendingFiles.length > 0) {
			const uploadedUrls: string[] = [];
			for (const file of pendingFiles) {
				const result = await uploadFile(file);
				if (result.error) {
					uploadError = result.error;
					messageInput = content; // Restore content
					sending = false;
					uploading = false;
					return;
				}
				if (result.data) {
					uploadedUrls.push(`[${result.data.filename}](${result.data.url})`);
				}
			}
			pendingFiles = [];
			// Append file links to message
			if (uploadedUrls.length > 0) {
				content = content ? `${content}\n\n${uploadedUrls.join('\n')}` : uploadedUrls.join('\n');
			}
		}

		uploading = false;

		const result = await sendMessage(serverId, channelId, content);

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

	// Typing indicator text
	let typingText = $derived.by(() => {
		const users = Array.from(typingUsers.values());
		if (users.length === 0) return '';
		if (users.length === 1) return `${users[0]} is typing...`;
		if (users.length === 2) return `${users[0]} and ${users[1]} are typing...`;
		return `${users[0]} and ${users.length - 1} others are typing...`;
	});
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

		{#if uploadError}
			<div class="upload-error">{uploadError}</div>
		{/if}

		{#if pendingFiles.length > 0}
			<div class="pending-files">
				{#each pendingFiles as file, i}
					<div class="pending-file">
						<span class="file-name">{file.name}</span>
						<span class="file-size">{formatFileSize(file.size)}</span>
						<button type="button" class="remove-file" onclick={() => removeFile(i)}>x</button>
					</div>
				{/each}
			</div>
		{/if}

		<form onsubmit={handleSend} class="input-form">
			<input
				type="file"
				bind:this={fileInput}
				onchange={handleFileSelect}
				multiple
				accept="image/*,video/*,audio/*,.pdf,.doc,.docx,.xls,.xlsx,.txt"
				hidden
			/>
			<button
				type="button"
				class="attach-btn"
				onclick={() => fileInput.click()}
				disabled={sending || uploading}
				title="Attach file"
			>
				+
			</button>
			<input
				type="text"
				class="message-input"
				placeholder="Message #{channel?.name ?? 'channel'}"
				bind:value={messageInput}
				oninput={handleTyping}
				disabled={sending || uploading}
			/>
			{#if uploading}
				<span class="uploading-text">Uploading...</span>
			{/if}
		</form>
	</div>
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

	/* Third dot via box-shadow */
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

	/* File upload styles */
	.input-form {
		display: flex;
		align-items: center;
		gap: 8px;
	}

	.attach-btn {
		width: 36px;
		height: 36px;
		border-radius: 50%;
		background: var(--bg-secondary);
		border: none;
		color: var(--text-muted);
		font-size: 20px;
		cursor: pointer;
		display: flex;
		align-items: center;
		justify-content: center;
		flex-shrink: 0;
	}

	.attach-btn:hover:not(:disabled) {
		background: var(--bg-tertiary);
		color: var(--text-primary);
	}

	.attach-btn:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}

	.pending-files {
		display: flex;
		flex-wrap: wrap;
		gap: 8px;
		padding: 8px 0;
	}

	.pending-file {
		display: flex;
		align-items: center;
		gap: 8px;
		background: var(--bg-secondary);
		padding: 6px 10px;
		border-radius: 4px;
		font-size: 13px;
	}

	.file-name {
		color: var(--text-primary);
		max-width: 150px;
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
	}

	.file-size {
		color: var(--text-muted);
		font-size: 11px;
	}

	.remove-file {
		background: none;
		border: none;
		color: var(--text-muted);
		cursor: pointer;
		padding: 0 4px;
		font-size: 14px;
	}

	.remove-file:hover {
		color: var(--danger);
	}

	.upload-error {
		color: var(--danger);
		font-size: 12px;
		padding: 4px 0;
	}

	.uploading-text {
		font-size: 12px;
		color: var(--text-muted);
		flex-shrink: 0;
	}
</style>
