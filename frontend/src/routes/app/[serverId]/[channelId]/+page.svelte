<script lang="ts">
	import { page } from '$app/stores';
	import { getContext } from 'svelte';
	import type { ServerWithChannels } from '$lib/api';

	const serverContext = getContext<{ server: ServerWithChannels | null }>('server');

	let channelId = $derived($page.params.channelId);
	let channel = $derived(serverContext.server?.channels.find((c) => c.id === channelId));
</script>

<div class="channel-view">
	<header class="channel-header">
		<span class="channel-hash">#</span>
		<span class="channel-name">{channel?.name ?? 'Loading...'}</span>
	</header>

	<div class="messages-container">
		<div class="empty-messages">
			<h3>Welcome to the channel!</h3>
			<p>This is the beginning of your conversation.</p>
			<p class="hint">Messaging coming soon...</p>
		</div>
	</div>

	<div class="message-input-container">
		<input
			type="text"
			class="message-input"
			placeholder="Message #{channel?.name ?? 'channel'} (coming soon)"
			disabled
		/>
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
		padding: 16px;
		display: flex;
		flex-direction: column;
	}

	.empty-messages {
		flex: 1;
		display: flex;
		flex-direction: column;
		align-items: center;
		justify-content: center;
		text-align: center;
		color: var(--text-muted);
	}

	.empty-messages h3 {
		color: var(--text-primary);
		margin-bottom: 8px;
	}

	.empty-messages .hint {
		margin-top: 16px;
		font-size: 12px;
		color: var(--text-muted);
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
		cursor: not-allowed;
	}
</style>
