import { test, expect } from '@playwright/test';

// Helper to register a unique user
async function registerUniqueUser(page: import('@playwright/test').Page) {
	const timestamp = Date.now();
	const user = {
		email: `msg-${timestamp}@example.com`,
		username: `msg${timestamp % 10000000}`,
		password: 'TestPassword123!',
	};

	await page.goto('/');
	await expect(page.locator('h1')).toContainText('Agorusta');
	await page.getByRole('button', { name: 'Register' }).click();
	await page.locator('#email').fill(user.email);
	await page.locator('#username').fill(user.username);
	await page.locator('#password').fill(user.password);
	await page.getByRole('button', { name: 'Register', exact: true }).click();
	await expect(page).toHaveURL(/\/app/, { timeout: 15000 });

	return user;
}

test.describe('Messaging', () => {
	test('should create a server and send a message', async ({ page }) => {
		await registerUniqueUser(page);

		const serverName = `MsgTest-${Date.now()}`;

		// Click create server button
		await page.locator('button[title="Create Server"]').click();

		// Fill server name in modal
		await page.locator('#server-name').fill(serverName);

		// Submit
		await page.getByRole('button', { name: 'Create' }).click();

		// Wait for navigation to server page
		await expect(page).toHaveURL(/\/app\/[^/]+$/, { timeout: 10000 });

		// Should see "Select a Channel" text
		await expect(page.getByText('Select a Channel')).toBeVisible({ timeout: 5000 });

		// Click on general channel in the sidebar
		await page.locator('.channel-link', { hasText: 'general' }).click();

		// Wait for channel page to load
		await expect(page).toHaveURL(/\/app\/[^/]+\/[^/]+/, { timeout: 5000 });

		// Send a message
		const testMessage = `Hello world ${Date.now()}`;
		await page.locator('input[placeholder*="Message"]').fill(testMessage);
		await page.keyboard.press('Enter');

		// Verify message appears
		await expect(page.getByText(testMessage)).toBeVisible({ timeout: 10000 });
	});

	test('should load message history after refresh', async ({ page }) => {
		await registerUniqueUser(page);

		const serverName = `History-${Date.now()}`;

		// Create server
		await page.locator('button[title="Create Server"]').click();
		await page.locator('#server-name').fill(serverName);
		await page.getByRole('button', { name: 'Create' }).click();

		// Wait for server page
		await expect(page).toHaveURL(/\/app\/[^/]+$/, { timeout: 10000 });

		// Click on general channel
		await page.locator('.channel-link', { hasText: 'general' }).click();
		await expect(page).toHaveURL(/\/app\/[^/]+\/[^/]+/, { timeout: 5000 });

		// Send multiple messages
		const messages = [`Msg1-${Date.now()}`, `Msg2-${Date.now()}`, `Msg3-${Date.now()}`];
		for (const msg of messages) {
			await page.locator('input[placeholder*="Message"]').fill(msg);
			await page.keyboard.press('Enter');
			await page.waitForTimeout(500);
		}

		// Verify messages appear
		for (const msg of messages) {
			await expect(page.getByText(msg)).toBeVisible({ timeout: 5000 });
		}

		// Refresh page
		await page.reload();
		await page.waitForTimeout(3000);

		// Messages should still be there
		for (const msg of messages) {
			await expect(page.getByText(msg)).toBeVisible({ timeout: 5000 });
		}
	});
});
