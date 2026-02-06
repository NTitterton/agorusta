import { test, expect } from '@playwright/test';

test.describe('Authentication', () => {
	const timestamp = Date.now();
	const testUser = {
		email: `test-${timestamp}@example.com`,
		username: `testuser${timestamp % 1000000}`,
		password: 'TestPassword123!',
	};

	test('should register a new user', async ({ page }) => {
		await page.goto('/');

		// Wait for page to load
		await expect(page.locator('h1')).toContainText('Agorusta');

		// Click "Register" button to switch to register mode
		await page.getByRole('button', { name: 'Register' }).click();

		// Fill registration form
		await page.locator('#email').fill(testUser.email);
		await page.locator('#username').fill(testUser.username);
		await page.locator('#password').fill(testUser.password);

		// Submit
		await page.getByRole('button', { name: 'Register', exact: true }).click();

		// Should redirect to app
		await expect(page).toHaveURL(/\/app/, { timeout: 10000 });
	});

	test('should login with existing user', async ({ page }) => {
		// First register a user
		await page.goto('/');
		await page.getByRole('button', { name: 'Register' }).click();

		const loginUser = {
			email: `login-${timestamp}@example.com`,
			username: `loginuser${timestamp % 1000000}`,
			password: 'TestPassword123!',
		};

		await page.locator('#email').fill(loginUser.email);
		await page.locator('#username').fill(loginUser.username);
		await page.locator('#password').fill(loginUser.password);
		await page.getByRole('button', { name: 'Register', exact: true }).click();
		await expect(page).toHaveURL(/\/app/, { timeout: 10000 });

		// Logout by clearing token and navigating back
		await page.evaluate(() => localStorage.removeItem('token'));
		await page.goto('/');

		// Wait for page to load
		await expect(page.locator('h1')).toContainText('Agorusta');

		// Login (should already be in login mode)
		await page.locator('#email').fill(loginUser.email);
		await page.locator('#password').fill(loginUser.password);
		await page.getByRole('button', { name: 'Login', exact: true }).click();

		// Should redirect to app
		await expect(page).toHaveURL(/\/app/, { timeout: 10000 });
	});

	test('should show error for invalid credentials', async ({ page }) => {
		await page.goto('/');

		// Wait for page to load
		await expect(page.locator('h1')).toContainText('Agorusta');

		await page.locator('#email').fill('nonexistent@example.com');
		await page.locator('#password').fill('wrongpassword');
		await page.getByRole('button', { name: 'Login', exact: true }).click();

		// Should show error message
		await expect(page.locator('.error')).toBeVisible({ timeout: 10000 });
	});
});
