<!-- The two buttons that settle a question asked in place, such as "Delete main?": the one
     that does it, and Cancel. The row they sit in asks the question. -->
<script lang="ts">
  import { Button } from "$lib/components/ui/button";

  type Props = {
    /** What the confirming button says, such as "Delete". */
    action: string;
    /** Whether confirming loses something. Deleting does; overwriting a profile does not. */
    destructive?: boolean;
    disabled?: boolean;
    onconfirm: () => void;
    oncancel: () => void;
  };
  let { action, destructive = true, disabled = false, onconfirm, oncancel }: Props = $props();
</script>

<Button variant={destructive ? "destructive" : "default"} size="sm" {disabled} onclick={onconfirm}>{action}</Button>
<Button variant="ghost" size="sm" onclick={oncancel}>Cancel</Button>
