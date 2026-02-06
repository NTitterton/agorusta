<script lang="ts">
	import { onMount } from 'svelte';
	import { getMembers, type Member } from '$lib/api';
	import { websocket } from '$lib/websocket.svelte';

	interface Props {
		serverId: string;
	}

	let { serverId }: Props = $props();

	let members = $state<Member[]>([]);
	let loading = $state(true);
	let error = $state<string | null>(null);

	// Group members by role
	let groupedMembers = $derived.by(() => {
		const groups: { owner: Member[]; admins: Member[]; members: Member[] } = {
			owner: [],
			admins: [],
			members: []
		};

		for (const member of members) {
			if (member.role === 'owner') {
				groups.owner.push(member);
			} else if (member.role === 'admin') {
				groups.admins.push(member);
			} else {
				groups.members.push(member);
			}
		}

		// Sort each group: online first, then alphabetically
		const sortFn = (a: Member, b: Member) => {
			if (a.is_online !== b.is_online) {
				return a.is_online ? -1 : 1;
			}
			return a.username.localeCompare(b.username);
		};

		groups.owner.sort(sortFn);
		groups.admins.sort(sortFn);
		groups.members.sort(sortFn);

		return groups;
	});

	// Count online members in each group
	let onlineCounts = $derived({
		owner: groupedMembers.owner.filter((m) => m.is_online).length,
		admins: groupedMembers.admins.filter((m) => m.is_online).length,
		members: groupedMembers.members.filter((m) => m.is_online).length
	});

	async function loadMembers() {
		loading = true;
		error = null;
		const result = await getMembers(serverId);
		if (result.data) {
			members = result.data;
		} else {
			error = result.error || 'Failed to load members';
		}
		loading = false;
	}

	// Handle presence updates
	function handlePresenceChange(userId: string, _username: string, isOnline: boolean) {
		const memberIndex = members.findIndex((m) => m.user_id === userId);
		if (memberIndex !== -1) {
			members = members.map((m) => (m.user_id === userId ? { ...m, is_online: isOnline } : m));
		}
	}

	$effect(() => {
		// Reload when serverId changes
		if (serverId) {
			loadMembers();
		}
	});

	onMount(() => {
		// Subscribe to presence updates
		const unsubscribe = websocket.subscribeToPresence(handlePresenceChange);

		return () => {
			unsubscribe();
		};
	});
</script>

<aside class="members-sidebar">
	{#if loading}
		<div class="members-loading">Loading members...</div>
	{:else if error}
		<div class="members-error">{error}</div>
	{:else}
		{#if groupedMembers.owner.length > 0}
			<div class="member-group">
				<div class="group-header">
					Owner - {onlineCounts.owner > 0 ? onlineCounts.owner : groupedMembers.owner.length}
				</div>
				{#each groupedMembers.owner as member}
					<div class="member-item" class:offline={!member.is_online}>
						<span class="status-dot" class:online={member.is_online}></span>
						<span class="member-name">{member.username}</span>
						<span class="role-badge owner">Owner</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if groupedMembers.admins.length > 0}
			<div class="member-group">
				<div class="group-header">
					Admins - {onlineCounts.admins > 0 ? onlineCounts.admins : groupedMembers.admins.length}
				</div>
				{#each groupedMembers.admins as member}
					<div class="member-item" class:offline={!member.is_online}>
						<span class="status-dot" class:online={member.is_online}></span>
						<span class="member-name">{member.username}</span>
						<span class="role-badge admin">Admin</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if groupedMembers.members.length > 0}
			<div class="member-group">
				<div class="group-header">
					Members - {onlineCounts.members > 0 ? onlineCounts.members : groupedMembers.members.length}
				</div>
				{#each groupedMembers.members as member}
					<div class="member-item" class:offline={!member.is_online}>
						<span class="status-dot" class:online={member.is_online}></span>
						<span class="member-name">{member.username}</span>
					</div>
				{/each}
			</div>
		{/if}

		{#if members.length === 0}
			<div class="members-empty">No members found</div>
		{/if}
	{/if}
</aside>

<style>
	.members-sidebar {
		width: 240px;
		background-color: var(--bg-secondary);
		display: flex;
		flex-direction: column;
		overflow-y: auto;
		padding: 16px 8px;
	}

	.members-loading,
	.members-error,
	.members-empty {
		font-size: 14px;
		color: var(--text-muted);
		padding: 8px;
		text-align: center;
	}

	.members-error {
		color: var(--danger);
	}

	.member-group {
		margin-bottom: 16px;
	}

	.group-header {
		font-size: 12px;
		text-transform: uppercase;
		color: var(--text-muted);
		font-weight: 600;
		letter-spacing: 0.5px;
		padding: 0 8px;
		margin-bottom: 4px;
	}

	.member-item {
		display: flex;
		align-items: center;
		padding: 6px 8px;
		border-radius: 4px;
		cursor: pointer;
		transition: background-color 0.15s ease;
	}

	.member-item:hover {
		background-color: var(--bg-primary);
	}

	.member-item.offline {
		opacity: 0.5;
	}

	.status-dot {
		width: 8px;
		height: 8px;
		border-radius: 50%;
		background-color: var(--text-muted);
		margin-right: 8px;
		flex-shrink: 0;
	}

	.status-dot.online {
		background-color: var(--success);
	}

	.member-name {
		font-size: 14px;
		color: var(--text-secondary);
		overflow: hidden;
		text-overflow: ellipsis;
		white-space: nowrap;
		flex: 1;
	}

	.member-item:hover .member-name {
		color: var(--text-primary);
	}

	.role-badge {
		font-size: 10px;
		padding: 2px 6px;
		border-radius: 3px;
		font-weight: 600;
		text-transform: uppercase;
		margin-left: 4px;
		flex-shrink: 0;
	}

	.role-badge.owner {
		background-color: var(--accent);
		color: var(--bg-primary);
	}

	.role-badge.admin {
		background-color: var(--border);
		color: var(--text-primary);
	}
</style>
