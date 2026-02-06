// Test user credentials - these should exist in your dev environment
// Or the tests will create them during setup
export const TEST_USERS = {
	user1: {
		email: 'e2e-user1@test.com',
		username: 'e2euser1',
		password: 'TestPassword123!',
	},
	user2: {
		email: 'e2e-user2@test.com',
		username: 'e2euser2',
		password: 'TestPassword123!',
	},
};

export const TEST_SERVER = {
	name: `E2E-Test-Server-${Date.now()}`,
};
