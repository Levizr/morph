// 20_cpp_types - c++ types in ts (int, int32, int64, float, double, std_string, etc.)
let a: int = 42;
console.log(a);
let b: int32 = 100;
console.log(b);
let c: int64 = 9999999999;
console.log(c);
let d: uint = 50;
console.log(d);
let e: uint32 = 60;
console.log(e);
let f: uint64 = 70;
console.log(f);
let g: float = 3.14;
console.log(g);
let h: double = 2.718;
console.log(h);
let i: char = 'A';
console.log(i);
let j: size_t = 123;
console.log(j);
let k: byte = 255;
console.log(k);

let s1: std_string = "hello std";
console.log(s1);
let s2: string = "hello JsString";
console.log(s2);

// vector types
let v1: std_vector<int> = [1, 2, 3];
console.log(v1);
let v2: std_vector<std_string> = ["a", "b"];
console.log(v2);
let v3: std_vector<JsNumber> = [1, 2, 3];
console.log(v3);

// optional and variant (if supported)
let opt: std_optional<int> = 5;
console.log(opt);

// number vs int
let n1: number = 10;
let n2: int = 20;
console.log(n1 + n2);
console.log(n1 > n2);

// string vs std_string
let jsStr: string = "js";
let cppStr: std_string = "cpp";
console.log(jsStr);
console.log(cppStr);

// bool vs boolean
let b1: boolean = true;
let b2: bool = false;
console.log(b1);
console.log(b2);

// any vs JsValue
let anyVal: any = 123;
console.log(anyVal);
let jsVal: JsValue = "test";
console.log(jsVal);

// Custom class with cpp type
class MyClass {
    value: int;
    constructor(v: int) {
        this.value = v;
    }
    getValue(): int {
        return this.value;
    }
}
let mc = new MyClass(42);
console.log(mc.getValue());

// Template with cpp types
function processInt(x: int): int {
    return x + 1;
}
console.log(processInt(5));

// Using size_t in loop
let arr: int[] = [1, 2, 3];
for (let idx: size_t = 0; idx < arr.length; idx++) {
    console.log(arr[idx]);
}
