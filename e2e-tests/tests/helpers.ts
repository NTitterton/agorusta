import { Page, expect } from '@playwright/test';

export async function registerUser(
	page: Page,
	email: string,
	username: string,
	password: string
): Promise<boolean> {
	await page.goto('/');

	// Check if we need to register or if already on app
	if (page.url().includes('/app')) {
		return true; // Already logged in
	}

	// Click register link if on login page
	const registerLink = page.getByRole('link', { name: /register/i });
	if (await registerLink.isVisible()) {
		await registerLink.click();
	}

	// Fill registration form
	await page.getByLabel(/email/i).fill(email);
	await page.getByLabel(/username/i).fill(username);
	await page.getByLabel(/password/i).fill(password);

	// Submit
	await page.getByRole('button', { name: /register|sign up/i }).click();

	// Wait for navigation to app or error
	try {
		await page.waitForURL(/\/app/, { timeout: 5000 });
		return true;
	} catch {
		// Registration might have failed (user exists), try login
		return false;
	}
}

export async function loginUser(
	page: Page,
	email: string,
	password: string
): Promise<void> {
	await page.goto('/');

	// If already logged in, return
	if (page.url().includes('/app')) {
		return;
	}

	// Fill login form
	await page.getByLabel(/email/i).fill(email);
	await page.getByLabel(/password/i).fill(password);

	// Submit
	await page.getByRole('button', { name: /login|sign in/i }).click();

	// Wait for navigation to app
	await page.waitForURL(/\/app/, { timeout: 10000 });
}

export async function ensureUserExists(
	page: Page,
	email: string,
	username: string,
	password: string
): Promise<void> {
	// Try to register, if fails try to login
	const registered = await registerUser(page, email, username, password);
	if (!registered) {
		await loginUser(page, email, password);
	}
}

export async function logout(page: Page): Promise<void> {
	// Clear localStorage and reload
	await page.evaluate(() => {
		localStorage.removeItem('token');
	});
	await page.goto('/');
}

export async function createServer(page: Page, serverName: string): Promise<void> {
	// Click create server button (the + button)
	await page.locator('button[title="Create Server"], .create-server-btn, button:has-text("+")').first().click();

	// Fill server name
	await page.getByLabel(/server name/i).fill(serverName);

	// Submit
	await page.getByRole('button', { name: /create/i }).click();

	// Wait for server to appear in sidebar
	await expect(page.getByText(serverName)).toBeVisible({ timeout: 10000 });
}

export async function waitForWebSocket(page: Page): Promise<void> {
	// Wait for WebSocket to connect by checking for connection indicator
	// or just wait a bit for the connection to establish
	await page.waitForTimeout(2000);
}

export async function getMembersList(page: Page): Promise<{ username: string; isOnline: boolean }[]> {
	const members: { username: string; isOnline: boolean }[] = [];

	// Get all member items in the sidebar
	const memberItems = page.locator('.member-item');
	const count = await memberItems.count();

	for (let i = 0; i < count; i++) {
		const item = memberItems.nth(i);
		const username = await item.locator('.member-name').textContent() || '';
		const statusDot = item.locator('.status-dot');
		const isOnline = await statusDot.evaluate((el) => el.classList.contains('online'));
		members.push({ username: username.trim(), isOnline });
	}

	return members;
}
