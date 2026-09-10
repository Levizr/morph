// 22_escape - escape analysis: stack, closure capture, return, aliasing
let localTotal: number = 10;
localTotal = localTotal + 5;
console.log(localTotal);

function makeCounter(): any {
    let count: number = 0;
    const bump = (): number => {
        count = count + 1;
        return count;
    };
    return bump();
}
console.log(makeCounter());

function doubleIt(n: number): number {
    let doubled: number = n * 2;
    return doubled;
}
console.log(doubleIt(21));

let shared: number = 1;
let alias: number = shared;
alias = alias + 9;
console.log(alias);
console.log(shared);
