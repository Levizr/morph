// 09_try_catch - try/catch, try/finally, throw, nested
function risky(shouldFail: boolean): string {
    if (shouldFail) {
        throw new Error("fail");
    }
    return "success";
}

function test1(): void {
    try {
        let r = risky(false);
        console.log(r);
    } catch (e) {
        console.log("caught");
    }
}
test1();

function test2(): void {
    try {
        risky(true);
    } catch (e) {
        console.log("caught error");
    }
}
test2();

function test3(): void {
    try {
        console.log("try");
    } catch (e) {
        console.log("catch for finally");
    } finally {
        console.log("finally");
    }
}
test3();

function test4(): void {
    try {
        throw new Error("test throw");
    } catch (e: any) {
        console.log(e["message"]);
    }
}
test4();

function mayThrow(x: number): number {
    if (x < 0) throw new Error("negative");
    return x * 2;
}
function test5(): void {
    try {
        console.log(mayThrow(5));
        console.log(mayThrow(-1));
    } catch (e) {
        console.log("negative caught");
    }
}
test5();
