// 16_loops - comprehensive loop tests
// for loop
let sum: number = 0;
for (let i: number = 0; i < 5; i++) {
    sum += i;
}
console.log(sum);

// for loop no init
let j: number = 0;
for (; j < 3; j++) {
    console.log(j);
}

// for loop no condition
let k: number = 0;
for (let k2: number = 0; ; k2++) {
    if (k2 > 2) break;
    console.log(k2);
}

// for loop no update
let m: number = 0;
for (let m2: number = 0; m2 < 3; ) {
    console.log(m2);
    m2++;
}

// while
let w: number = 0;
while (w < 3) {
    console.log(w);
    w++;
}

// do-while
let d: number = 0;
do {
    console.log(d);
    d++;
} while (d < 2);

// for...of with string array
let arr: string[] = ["a", "b", "c"];
let total: string = "";
for (let val of arr) {
    total = total + val;
}
console.log(total);

// for...of with number array
let nums: number[] = [1, 2, 3];
let nsum: number = 0;
for (let n of nums) {
    nsum = nsum + n;
}
console.log(nsum);

// for...in (object keys)
let obj = {a: 1, b: 2};
for (let key in obj) {
    console.log(key);
}

// nested loops
let nestedSum: number = 0;
for (let i: number = 0; i < 2; i++) {
    for (let j: number = 0; j < 2; j++) {
        nestedSum += 1;
    }
}
console.log(nestedSum);

// break
for (let i: number = 0; i < 10; i++) {
    if (i === 3) break;
    console.log(i);
}

// continue
for (let i: number = 0; i < 5; i++) {
    if (i % 2 === 0) continue;
    console.log(i);
}

// while with break
let b2: number = 0;
while (true) {
    if (b2 >= 2) break;
    console.log(b2);
    b2++;
}

// do-while with continue
let c: number = 0;
do {
    c++;
    if (c === 2) continue;
    console.log(c);
} while (c < 3);
