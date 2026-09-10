// 28_last_use_move - values move (not copy) at their final read:
// call arguments, assignment sources, and returned locals.
function take(text: string): void {
    console.log(text);
}

function echo(text: string): string {
    console.log(text);
    return text;
}

async function main(): Promise<void> {
    let greeting: string = "hello";
    console.log(greeting);
    take(greeting);
    let label: string = "lbl";
    console.log(label);
    let backup: string = "";
    backup = label;
    console.log(backup);
    console.log(echo("hi"));
}
main();
