// 21_js_comparison - intent-based JS equality with native C++ types
let emptyText: string = "";
let userName: string = "Ada";
let answerCount: number = 42;
let zeroCount: number = 0;
let isReady: boolean = true;
let isDone: boolean = false;

console.log(emptyText == zeroCount);
console.log(userName == answerCount);
console.log("42" == answerCount);
console.log("  7  " == 7);
console.log("abc" == 1);
console.log(isReady == 1);
console.log(isDone == zeroCount);
console.log(isReady == 2);
console.log(userName == userName);
console.log(emptyText == emptyText);

console.log("42" === answerCount);
console.log(answerCount === 42);
console.log(isReady === 1);
console.log(emptyText === "");
console.log(null === undefined);

console.log(null == undefined);
console.log(null == zeroCount);

console.log("10" > 2);
console.log("2" < "10");
console.log("b" > "a");
console.log(answerCount > zeroCount);

if (emptyText) {
    console.log("empty is truthy");
} else {
    console.log("empty is falsy");
}
if (userName) {
    console.log("name is truthy");
}
if (zeroCount) {
    console.log("zero is truthy");
} else {
    console.log("zero is falsy");
}
if (answerCount) {
    console.log("count is truthy");
}
console.log(!emptyText);
console.log(!userName);
console.log(!isReady);

console.log(userName && "fallback");
console.log(emptyText || "fallback");

let maxCount: number = answerCount > zeroCount ? answerCount : zeroCount;
console.log(maxCount);
let label: string = emptyText ? "has-text" : "no-text";
console.log(label);
