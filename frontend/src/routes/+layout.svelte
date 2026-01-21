<script lang="ts">
	import '../app.css';

	let { children } = $props();
</script>

<svelte:head>
	<title>Agorusta</title>
</svelte:head>

{@render children()}
