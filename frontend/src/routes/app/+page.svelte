<script lang="ts">
	// Empty page - user should select or create a server
</script>

<div class="no-server">
	<div class="empty-state">
		<h2>Welcome to Agorusta</h2>
		<p>Select a server from the sidebar or create a new one to get started.</p>
	</div>
</div>

<style>
	.no-server {
		flex: 1;
		display: flex;
		align-items: center;
		justify-content: center;
		background-color: var(--bg-primary);
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
