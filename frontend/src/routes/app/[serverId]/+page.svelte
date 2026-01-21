<script lang="ts">
	// Server selected but no channel - prompt to select a channel
</script>

<div class="no-channel">
	<div class="empty-state">
		<h2>Select a Channel</h2>
		<p>Choose a channel from the sidebar to start chatting.</p>
	</div>
</div>

<style>
	.no-channel {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
	}

	.empty-state {
		text-align: center;
		color: var(--text-muted);
	}

	.empty-state h2 {
		color: var(--text-primary);
		margin-bottom: 8px;
	}
</style>
