// 05_objects - literals, shorthand, computed, nested, methods
let obj1: any = {a: 1, b: 2};
console.log(obj1["a"]);
console.log(obj1["b"]);

let x: number = 10;
let y: number = 20;
let shorthand = {x, y};
console.log(shorthand["x"]);
console.log(shorthand["y"]);

let nested = {inner: {value: 42}, name: "test"};
console.log(nested["inner"]["value"]);
console.log(nested["name"]);

let emptyObj = {};
console.log(emptyObj);

let arrInObj = {arr: [1, 2, 3], count: 3};
console.log(arrInObj["arr"].length);
console.log(arrInObj["count"]);

let simpleObj = {name: "test", value: 123};
console.log(simpleObj["name"]);
console.log(simpleObj["value"]);
