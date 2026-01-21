<script lang="ts">
	import { page } from '$app/stores';
	import { getContext, onMount } from 'svelte';
	import {
		listInvites,
		createInvite,
		deleteInvite,
		listServerPasswords,
		createServerPassword,
		deleteServerPassword,
		type Invite,
		type ServerPassword,
		type ServerWithChannels
	} from '$lib/api';

	const serverContext = getContext<{ server: ServerWithChannels | null }>('server');

	let invites = $state<Invite[]>([]);
	let passwords = $state<ServerPassword[]>([]);
	let loadingInvites = $state(true);
	let loadingPasswords = $state(true);
	let error = $state('');

	// Create invite form
	let showCreateInvite = $state(false);
	let inviteExpiry = $state<string>('never');
	let inviteMaxUses = $state<string>('unlimited');
	let creatingInvite = $state(false);

	// Create password form
	let showCreatePassword = $state(false);
	let newPassword = $state('');
	let passwordExpiry = $state<string>('never');
	let creatingPassword = $state(false);

	// Copied feedback
	let copiedCode = $state<string | null>(null);

	$effect(() => {
		if (serverContext.server) {
			loadInvites();
			loadPasswords();
		}
	});

	async function loadInvites() {
		if (!serverContext.server) return;
		loadingInvites = true;
		const result = await listInvites(serverContext.server.id);
		if (result.data) {
			invites = result.data;
		}
		loadingInvites = false;
	}

	async function loadPasswords() {
		if (!serverContext.server) return;
		loadingPasswords = true;
		const result = await listServerPasswords(serverContext.server.id);
		if (result.data) {
			passwords = result.data;
		}
		loadingPasswords = false;
	}

	async function handleCreateInvite(e: Event) {
		e.preventDefault();
		if (!serverContext.server) return;

		creatingInvite = true;
		error = '';

		const options: { expires_in_hours?: number; max_uses?: number } = {};

		if (inviteExpiry !== 'never') {
			options.expires_in_hours = parseInt(inviteExpiry);
		}
		if (inviteMaxUses !== 'unlimited') {
			options.max_uses = parseInt(inviteMaxUses);
		}

		const result = await createInvite(serverContext.server.id, options);
		creatingInvite = false;

		if (result.error) {
			error = result.error;
		} else if (result.data) {
			invites = [result.data, ...invites];
			showCreateInvite = false;
			inviteExpiry = 'never';
			inviteMaxUses = 'unlimited';
		}
	}

	async function handleDeleteInvite(code: string) {
		if (!serverContext.server) return;

		const result = await deleteInvite(serverContext.server.id, code);
		if (!result.error) {
			invites = invites.filter((i) => i.code !== code);
		}
	}

	async function handleCreatePassword(e: Event) {
		e.preventDefault();
		if (!serverContext.server || !newPassword) return;

		creatingPassword = true;
		error = '';

		const expiresInHours = passwordExpiry !== 'never' ? parseInt(passwordExpiry) : undefined;

		const result = await createServerPassword(serverContext.server.id, newPassword, expiresInHours);
		creatingPassword = false;

		if (result.error) {
			error = result.error;
		} else if (result.data) {
			passwords = [result.data, ...passwords];
			showCreatePassword = false;
			newPassword = '';
			passwordExpiry = 'never';
		}
	}

	async function handleDeletePassword(passwordId: string) {
		if (!serverContext.server) return;

		const result = await deleteServerPassword(serverContext.server.id, passwordId);
		if (!result.error) {
			passwords = passwords.filter((p) => p.id !== passwordId);
		}
	}

	function copyToClipboard(code: string) {
		navigator.clipboard.writeText(code);
		copiedCode = code;
		setTimeout(() => {
			copiedCode = null;
		}, 2000);
	}

	function formatDate(timestamp: number): string {
		return new Date(timestamp * 1000).toLocaleString();
	}

	function formatExpiry(expiresAt: number | null): string {
		if (!expiresAt) return 'Never';
		const now = Date.now() / 1000;
		const remaining = expiresAt - now;
		if (remaining < 0) return 'Expired';
		if (remaining < 3600) return `${Math.floor(remaining / 60)}m remaining`;
		if (remaining < 86400) return `${Math.floor(remaining / 3600)}h remaining`;
		return `${Math.floor(remaining / 86400)}d remaining`;
	}
</script>

<div class="settings-page">
	<div class="settings-header">
		<h1>Server Settings</h1>
		<a href="/app/{$page.params.serverId}" class="back-link">Back to server</a>
	</div>

	{#if error}
		<div class="error-message">{error}</div>
	{/if}

	<!-- Invite Codes Section -->
	<section class="settings-section">
		<div class="section-header">
			<h2>Invite Codes</h2>
			<button class="primary small" onclick={() => (showCreateInvite = true)}>
				Create Invite
			</button>
		</div>

		{#if showCreateInvite}
			<form class="create-form" onsubmit={handleCreateInvite}>
				<div class="form-row">
					<div class="form-group">
						<label for="invite-expiry">Expires After</label>
						<select id="invite-expiry" bind:value={inviteExpiry}>
							<option value="never">Never</option>
							<option value="1">1 hour</option>
							<option value="24">24 hours</option>
							<option value="168">7 days</option>
							<option value="720">30 days</option>
						</select>
					</div>
					<div class="form-group">
						<label for="invite-max-uses">Max Uses</label>
						<select id="invite-max-uses" bind:value={inviteMaxUses}>
							<option value="unlimited">Unlimited</option>
							<option value="1">1 use</option>
							<option value="5">5 uses</option>
							<option value="10">10 uses</option>
							<option value="25">25 uses</option>
							<option value="100">100 uses</option>
						</select>
					</div>
				</div>
				<div class="form-actions">
					<button type="button" class="secondary small" onclick={() => (showCreateInvite = false)}>
						Cancel
					</button>
					<button type="submit" class="primary small" disabled={creatingInvite}>
						{creatingInvite ? 'Creating...' : 'Create'}
					</button>
				</div>
			</form>
		{/if}

		{#if loadingInvites}
			<p class="loading">Loading invites...</p>
		{:else if invites.length === 0}
			<p class="empty">No active invites. Create one to let people join your server.</p>
		{:else}
			<div class="items-list">
				{#each invites as invite}
					<div class="list-item">
						<div class="item-main">
							<code class="invite-code">{invite.code}</code>
							<button
								class="copy-btn"
								onclick={() => copyToClipboard(invite.code)}
								title="Copy code"
							>
								{copiedCode === invite.code ? 'Copied!' : 'Copy'}
							</button>
						</div>
						<div class="item-meta">
							<span>Uses: {invite.use_count}{invite.max_uses ? `/${invite.max_uses}` : ''}</span>
							<span>{formatExpiry(invite.expires_at)}</span>
						</div>
						<button class="delete-btn" onclick={() => handleDeleteInvite(invite.code)} title="Delete invite">
							Delete
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</section>

	<!-- Server Passwords Section -->
	<section class="settings-section">
		<div class="section-header">
			<h2>Server Passwords</h2>
			<button class="primary small" onclick={() => (showCreatePassword = true)}>
				Create Password
			</button>
		</div>
		<p class="section-description">
			Users can join your server by entering the server name and one of these passwords.
		</p>

		{#if showCreatePassword}
			<form class="create-form" onsubmit={handleCreatePassword}>
				<div class="form-row">
					<div class="form-group flex-1">
						<label for="new-password">Password</label>
						<input
							type="text"
							id="new-password"
							bind:value={newPassword}
							placeholder="Enter a password"
							required
							minlength="4"
						/>
					</div>
					<div class="form-group">
						<label for="password-expiry">Expires After</label>
						<select id="password-expiry" bind:value={passwordExpiry}>
							<option value="never">Never</option>
							<option value="1">1 hour</option>
							<option value="24">24 hours</option>
							<option value="168">7 days</option>
							<option value="720">30 days</option>
						</select>
					</div>
				</div>
				<div class="form-actions">
					<button type="button" class="secondary small" onclick={() => (showCreatePassword = false)}>
						Cancel
					</button>
					<button type="submit" class="primary small" disabled={creatingPassword || !newPassword}>
						{creatingPassword ? 'Creating...' : 'Create'}
					</button>
				</div>
			</form>
		{/if}

		{#if loadingPasswords}
			<p class="loading">Loading passwords...</p>
		{:else if passwords.length === 0}
			<p class="empty">No passwords set. Create one to allow name+password joining.</p>
		{:else}
			<div class="items-list">
				{#each passwords as pwd}
					<div class="list-item">
						<div class="item-main">
							<span class="password-label">Password #{passwords.indexOf(pwd) + 1}</span>
							<span class="created-at">Created {formatDate(pwd.created_at)}</span>
						</div>
						<div class="item-meta">
							<span>{formatExpiry(pwd.expires_at)}</span>
						</div>
						<button class="delete-btn" onclick={() => handleDeletePassword(pwd.id)} title="Delete password">
							Delete
						</button>
					</div>
				{/each}
			</div>
		{/if}
	</section>
</div>

<style>
	.settings-page {
		padding: 24px;
		max-width: 800px;
		margin: 0 auto;
		overflow-y: auto;
		height: 100%;
	}

	.settings-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 24px;
	}

	.settings-header h1 {
		font-size: 24px;
		margin: 0;
	}

	.back-link {
		color: var(--accent);
		text-decoration: none;
		font-size: 14px;
	}

	.back-link:hover {
		text-decoration: underline;
	}

	.error-message {
		background: rgba(237, 66, 69, 0.1);
		border: 1px solid var(--danger);
		color: var(--danger);
		padding: 10px 12px;
		border-radius: 4px;
		margin-bottom: 16px;
		font-size: 14px;
	}

	.settings-section {
		background: var(--bg-secondary);
		border-radius: 8px;
		padding: 20px;
		margin-bottom: 20px;
	}

	.section-header {
		display: flex;
		align-items: center;
		justify-content: space-between;
		margin-bottom: 12px;
	}

	.section-header h2 {
		font-size: 18px;
		margin: 0;
	}

	.section-description {
		font-size: 14px;
		color: var(--text-muted);
		margin-bottom: 16px;
	}

	.create-form {
		background: var(--bg-primary);
		padding: 16px;
		border-radius: 8px;
		margin-bottom: 16px;
	}

	.form-row {
		display: flex;
		gap: 16px;
		margin-bottom: 12px;
	}

	.form-group {
		display: flex;
		flex-direction: column;
		gap: 6px;
	}

	.form-group.flex-1 {
		flex: 1;
	}

	.form-group label {
		font-size: 12px;
		text-transform: uppercase;
		color: var(--text-muted);
		letter-spacing: 0.5px;
	}

	.form-group select,
	.form-group input {
		padding: 8px 12px;
		font-size: 14px;
	}

	.form-actions {
		display: flex;
		justify-content: flex-end;
		gap: 8px;
	}

	button.small {
		padding: 8px 16px;
		font-size: 13px;
	}

	.loading,
	.empty {
		font-size: 14px;
		color: var(--text-muted);
		text-align: center;
		padding: 20px;
	}

	.items-list {
		display: flex;
		flex-direction: column;
		gap: 8px;
	}

	.list-item {
		display: flex;
		align-items: center;
		gap: 16px;
		padding: 12px 16px;
		background: var(--bg-primary);
		border-radius: 6px;
	}

	.item-main {
		flex: 1;
		display: flex;
		align-items: center;
		gap: 12px;
	}

	.invite-code {
		font-family: monospace;
		font-size: 16px;
		background: var(--bg-tertiary);
		padding: 4px 8px;
		border-radius: 4px;
	}

	.copy-btn {
		background: none;
		border: 1px solid var(--border);
		color: var(--text-muted);
		padding: 4px 10px;
		font-size: 12px;
		border-radius: 4px;
		cursor: pointer;
	}

	.copy-btn:hover {
		border-color: var(--accent);
		color: var(--accent);
	}

	.password-label {
		font-weight: 500;
	}

	.created-at {
		font-size: 12px;
		color: var(--text-muted);
	}

	.item-meta {
		display: flex;
		gap: 16px;
		font-size: 12px;
		color: var(--text-muted);
	}

	.delete-btn {
		background: none;
		border: none;
		color: var(--danger);
		font-size: 12px;
		cursor: pointer;
		padding: 4px 8px;
		opacity: 0.7;
	}

	.delete-btn:hover {
		opacity: 1;
	}
</style>
