<script lang="ts">
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { checkHealth, API_URL, WS_URL } from '$lib/api';
	import { auth } from '$lib/auth.svelte';

	let apiStatus = $state<'checking' | 'online' | 'offline'>('checking');
	let statusError = $state<string | null>(null);

	let mode = $state<'login' | 'register'>('login');
	let email = $state('');
	let username = $state('');
	let password = $state('');
	let submitting = $state(false);

	onMount(async () => {
		// Check API health
		const result = await checkHealth();
		if (result.ok) {
			apiStatus = 'online';
		} else {
			apiStatus = 'offline';
			statusError = result.error || 'Failed to connect';
		}

		// Initialize auth state
		await auth.init();

		// Redirect to app if already authenticated
		if (auth.isAuthenticated) {
			goto('/app');
		}
	});

	async function handleSubmit(e: Event) {
		e.preventDefault();
		submitting = true;

		let success: boolean;
		if (mode === 'login') {
			success = await auth.login(email, password);
		} else {
			success = await auth.register(email, username, password);
		}

		submitting = false;

		if (success) {
			email = '';
			username = '';
			password = '';
			goto('/app');
		}
	}

	function handleLogout() {
		auth.logout();
	}

	function switchMode() {
		mode = mode === 'login' ? 'register' : 'login';
		auth.error = null;
	}
</script>

<div class="landing">
	<div class="hero">
		<h1>Agorusta</h1>
		<p class="tagline">A Discord-like chat app built with Rust + SvelteKit</p>

		<div class="status-card card">
			<h3>System Status</h3>
			<div class="status-row">
				<span class="status-dot" class:online={apiStatus === 'online'} class:offline={apiStatus === 'offline'}></span>
				<span>
					API: {apiStatus === 'checking' ? 'Checking...' : apiStatus}
				</span>
			</div>
			{#if statusError}
				<p class="error">{statusError}</p>
			{/if}
		</div>

		{#if auth.loading}
			<div class="card">
				<p>Loading...</p>
			</div>
		{:else if auth.isAuthenticated}
			<div class="card user-card">
				<h3>Welcome back!</h3>
				<div class="user-info">
					<p><strong>{auth.user?.username}</strong></p>
					<p class="email">{auth.user?.email}</p>
				</div>
				<button class="secondary" onclick={handleLogout}>Logout</button>
			</div>

			<div class="card">
				<p class="hint">Chat features coming soon...</p>
			</div>
		{:else}
			<div class="card auth-card">
				<h3>{mode === 'login' ? 'Login' : 'Create Account'}</h3>

				<form onsubmit={handleSubmit}>
					<div class="form-group">
						<label for="email">Email</label>
						<input
							type="email"
							id="email"
							bind:value={email}
							placeholder="you@example.com"
							required
						/>
					</div>

					{#if mode === 'register'}
						<div class="form-group">
							<label for="username">Username</label>
							<input
								type="text"
								id="username"
								bind:value={username}
								placeholder="cooluser123"
								minlength="3"
								required
							/>
						</div>
					{/if}

					<div class="form-group">
						<label for="password">Password</label>
						<input
							type="password"
							id="password"
							bind:value={password}
							placeholder="••••••••"
							minlength="8"
							required
						/>
					</div>

					{#if auth.error}
						<p class="error">{auth.error}</p>
					{/if}

					<button type="submit" class="primary" disabled={submitting}>
						{submitting ? 'Please wait...' : mode === 'login' ? 'Login' : 'Register'}
					</button>
				</form>

				<p class="switch-mode">
					{mode === 'login' ? "Don't have an account?" : 'Already have an account?'}
					<button class="link" onclick={switchMode}>
						{mode === 'login' ? 'Register' : 'Login'}
					</button>
				</p>
			</div>
		{/if}

		<div class="endpoints card">
			<h3>Endpoints</h3>
			<div class="endpoint">
				<span class="label">REST API:</span>
				<code>{API_URL}</code>
			</div>
			<div class="endpoint">
				<span class="label">WebSocket:</span>
				<code>{WS_URL}</code>
			</div>
		</div>
	</div>
</div>

<style>
	.landing {
		min-height: 100vh;
		display: flex;
		align-items: center;
		justify-content: center;
		padding: 20px;
	}

	.hero {
		text-align: center;
		max-width: 400px;
		width: 100%;
	}

	h1 {
		font-size: 3rem;
		margin-bottom: 8px;
		background: linear-gradient(135deg, #0ea5e9, var(--accent));
		-webkit-background-clip: text;
		-webkit-text-fill-color: transparent;
		background-clip: text;
	}

	.tagline {
		color: var(--text-secondary);
		margin-bottom: 32px;
	}

	.status-card,
	.endpoints,
	.auth-card,
	.user-card {
		text-align: left;
		margin-bottom: 24px;
	}

	h3 {
		font-size: 12px;
		text-transform: uppercase;
		color: var(--text-muted);
		margin-bottom: 12px;
		letter-spacing: 0.5px;
	}

	.status-row {
		display: flex;
		align-items: center;
		font-size: 14px;
	}

	.error {
		color: var(--danger);
		font-size: 12px;
		margin-top: 8px;
	}

	.endpoint {
		display: flex;
		align-items: center;
		gap: 8px;
		margin-bottom: 8px;
		font-size: 14px;
	}

	.label {
		color: var(--text-muted);
		min-width: 80px;
	}

	code {
		background-color: var(--bg-tertiary);
		padding: 4px 8px;
		border-radius: 4px;
		font-size: 12px;
		color: var(--text-secondary);
		word-break: break-all;
	}

	/* Auth form styles */
	.auth-card h3 {
		font-size: 16px;
		text-transform: none;
		color: var(--text-primary);
		margin-bottom: 16px;
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

	.form-group input {
		width: 100%;
	}

	form button[type='submit'] {
		width: 100%;
		margin-top: 8px;
	}

	.switch-mode {
		text-align: center;
		margin-top: 16px;
		font-size: 14px;
		color: var(--text-muted);
	}

	button.link {
		background: none;
		color: var(--accent);
		padding: 0;
		font-size: inherit;
	}

	button.link:hover {
		text-decoration: underline;
	}

	/* User card styles */
	.user-card h3 {
		font-size: 16px;
		text-transform: none;
		color: var(--text-primary);
	}

	.user-info {
		margin-bottom: 16px;
	}

	.user-info p {
		margin: 4px 0;
	}

	.user-info .email {
		color: var(--text-muted);
		font-size: 14px;
	}

	.hint {
		color: var(--text-muted);
		font-size: 14px;
		text-align: center;
	}

	button:disabled {
		opacity: 0.5;
		cursor: not-allowed;
	}
</style>
