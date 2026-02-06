import { test, expect } from '@playwright/test';

test.describe('User Presence', () => {
	const timestamp = Date.now();
	const testUser = {
		email: `pres-${timestamp}@test.com`,
		username: `pres${timestamp % 1000000}`,
		password: 'TestPassword123!',
	};
	const serverName = `PresTest-${timestamp}`;

	test('current user should always show as online in members sidebar', async ({ page }) => {
		// Register
		await page.goto('/');
		await page.getByRole('button', { name: 'Register' }).click();
		await page.locator('#email').fill(testUser.email);
		await page.locator('#username').fill(testUser.username);
		await page.locator('#password').fill(testUser.password);
		await page.getByRole('button', { name: 'Register', exact: true }).click();
		await expect(page).toHaveURL(/\/app/, { timeout: 15000 });

		// Create a server
		await page.locator('button[title="Create Server"]').click();
		await page.locator('#server-name').fill(serverName);
		await page.getByRole('button', { name: 'Create' }).click();
		await expect(page).toHaveURL(/\/app\/[^/]+$/, { timeout: 10000 });

		// Wait for members sidebar to load
		await page.waitForTimeout(3000);

		// Check that the current user appears in the members sidebar
		const memberItem = page.locator('.member-item', { hasText: testUser.username });
		await expect(memberItem).toBeVisible({ timeout: 5000 });

		// Check the status dot has 'online' class
		const statusDot = memberItem.locator('.status-dot');
		await expect(statusDot).toHaveClass(/online/, { timeout: 5000 });
	});

	test('user status should persist after page refresh', async ({ page }) => {
		const user2 = {
			email: `pres2-${timestamp}@test.com`,
			username: `pres2${timestamp % 1000000}`,
			password: 'TestPassword123!',
		};
		const server2 = `Persist-${timestamp}`;

		// Register
		await page.goto('/');
		await page.getByRole('button', { name: 'Register' }).click();
		await page.locator('#email').fill(user2.email);
		await page.locator('#username').fill(user2.username);
		await page.locator('#password').fill(user2.password);
		await page.getByRole('button', { name: 'Register', exact: true }).click();
		await expect(page).toHaveURL(/\/app/, { timeout: 15000 });

		// Create a server
		await page.locator('button[title="Create Server"]').click();
		await page.locator('#server-name').fill(server2);
		await page.getByRole('button', { name: 'Create' }).click();
		await expect(page).toHaveURL(/\/app\/[^/]+$/, { timeout: 10000 });

		// Wait for page to stabilize
		await page.waitForTimeout(3000);

		// Verify user is online
		const memberItem = page.locator('.member-item', { hasText: user2.username });
		await expect(memberItem).toBeVisible({ timeout: 5000 });
		await expect(memberItem.locator('.status-dot')).toHaveClass(/online/);

		// Refresh the page
		await page.reload();
		await page.waitForTimeout(3000);

		// User should still be online after refresh
		const memberItemAfterRefresh = page.locator('.member-item', { hasText: user2.username });
		await expect(memberItemAfterRefresh).toBeVisible({ timeout: 5000 });
		await expect(memberItemAfterRefresh.locator('.status-dot')).toHaveClass(/online/);
	});

	test('members sidebar should show role badges', async ({ page }) => {
		const user3 = {
			email: `pres3-${timestamp}@test.com`,
			username: `pres3${timestamp % 1000000}`,
			password: 'TestPassword123!',
		};
		const server3 = `Roles-${timestamp}`;

		// Register
		await page.goto('/');
		await page.getByRole('button', { name: 'Register' }).click();
		await page.locator('#email').fill(user3.email);
		await page.locator('#username').fill(user3.username);
		await page.locator('#password').fill(user3.password);
		await page.getByRole('button', { name: 'Register', exact: true }).click();
		await expect(page).toHaveURL(/\/app/, { timeout: 15000 });

		// Create a server
		await page.locator('button[title="Create Server"]').click();
		await page.locator('#server-name').fill(server3);
		await page.getByRole('button', { name: 'Create' }).click();
		await expect(page).toHaveURL(/\/app\/[^/]+$/, { timeout: 10000 });

		// Wait for members sidebar
		await page.waitForTimeout(3000);

		// Check for the "OWNER" group header
		await expect(page.locator('.group-header', { hasText: /OWNER/i })).toBeVisible({ timeout: 5000 });

		// Check that the current user has the Owner badge
		const memberItem = page.locator('.member-item', { hasText: user3.username });
		await expect(memberItem.locator('.role-badge')).toContainText(/Owner/i);
	});
});
