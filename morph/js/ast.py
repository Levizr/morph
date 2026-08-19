from __future__ import annotations
from dataclasses import dataclass, field
from typing import Optional


@dataclass
class TSType:
    raw: str


@dataclass
class TSPredefinedType(TSType):
    pass


@dataclass
class TSArrayType(TSType):
    element_type: TSType


@dataclass
class TSGenericType(TSType):
    name: str
    type_args: list[TSType]


@dataclass
class TSUnionType(TSType):
    types: list[TSType]


class TSNode:
    pass


@dataclass
class TSProgram(TSNode):
    statements: list[TSNode]


@dataclass
class TSIdentifier(TSNode):
    name: str
    type_annotation: Optional[TSType] = None


@dataclass
class TSLiteral(TSNode):
    value: str | int | float | bool | None
    raw: str


@dataclass
class TSTemplateLiteral(TSNode):
    parts: list[str | TSNode]


@dataclass
class TSVariableDeclaration(TSNode):
    kind: str
    name: TSIdentifier
    initializer: Optional[TSNode]
    type_annotation: Optional[TSType]


@dataclass
class TSFunctionDeclaration(TSNode):
    name: str
    params: list[TSVariableDeclaration]
    return_type: Optional[TSType]
    body: TSBlockStatement
    type_parameters: Optional[list[TSTypeParameter]] = None
    is_async: bool = False


@dataclass
class TSArrowFunction(TSNode):
    params: list[TSVariableDeclaration]
    return_type: Optional[TSType]
    body: TSBlockStatement | TSNode
    type_parameters: Optional[list[TSTypeParameter]] = None
    is_async: bool = False


@dataclass
class TSBlockStatement(TSNode):
    statements: list[TSNode]


@dataclass
class TSExpressionStatement(TSNode):
    expression: TSNode


@dataclass
class TSReturnStatement(TSNode):
    argument: Optional[TSNode]


@dataclass
class TSIfStatement(TSNode):
    condition: TSNode
    consequence: TSNode
    alternate: Optional[TSNode]


@dataclass
class TSWhileStatement(TSNode):
    condition: TSNode
    body: TSNode


@dataclass
class TSForStatement(TSNode):
    init: Optional[TSNode]
    condition: Optional[TSNode]
    update: Optional[TSNode]
    body: TSNode


@dataclass
class TSBinaryExpression(TSNode):
    operator: str
    left: TSNode
    right: TSNode


@dataclass
class TSUnaryExpression(TSNode):
    operator: str
    argument: TSNode
    prefix: bool


@dataclass
class TSUpdateExpression(TSNode):
    operator: str
    argument: TSNode
    prefix: bool


@dataclass
class TSAssignmentExpression(TSNode):
    operator: str
    left: TSNode
    right: TSNode


@dataclass
class TSCallExpression(TSNode):
    callee: TSNode
    arguments: list[TSNode]
    type_arguments: Optional[list[TSType]] = None


@dataclass
class TSAwaitExpression(TSNode):
    argument: TSNode


@dataclass
class TSMemberExpression(TSNode):
    object: TSNode
    property: TSNode
    computed: bool


@dataclass
class TSParenthesizedExpression(TSNode):
    expression: TSNode


@dataclass
class TSArrayLiteral(TSNode):
    elements: list[TSNode]


@dataclass
class TSSpreadElement(TSNode):
    argument: TSNode


@dataclass
class TSObjectProperty(TSNode):
    key: str
    value: TSNode


@dataclass
class TSObjectLiteral(TSNode):
    properties: list[TSObjectProperty]


@dataclass
class TSFunctionExpression(TSNode):
    params: list[TSVariableDeclaration]
    return_type: Optional[TSType]
    body: TSBlockStatement | TSNode
    type_parameters: Optional[list[TSTypeParameter]] = None
    is_async: bool = False


# ── OOP & Advanced ─────────────────────────────────────────


@dataclass
class TSThisExpression(TSNode):
    pass


@dataclass
class TSSuperExpression(TSNode):
    pass


@dataclass
class TSNewExpression(TSNode):
    callee: TSNode
    arguments: list[TSNode]
    type_arguments: Optional[list[TSType]] = None


@dataclass
class TSTypeParameter(TSNode):
    name: str
    constraint: Optional[TSType] = None


@dataclass
class TSPropertyDefinition(TSNode):
    name: str
    type_annotation: Optional[TSType]
    initializer: Optional[TSNode]
    access_modifier: str = ""
    static: bool = False
    readonly: bool = False


@dataclass
class TSMethodDefinition(TSNode):
    name: str
    params: list[TSVariableDeclaration]
    return_type: Optional[TSType]
    body: TSBlockStatement
    access_modifier: str = ""
    static: bool = False
    is_async: bool = False


@dataclass
class TSConstructor(TSNode):
    params: list[TSVariableDeclaration]
    body: TSBlockStatement
    access_modifier: str = "public"


@dataclass
class TSClassDeclaration(TSNode):
    name: str
    members: list[TSPropertyDefinition | TSMethodDefinition | TSConstructor]
    type_parameters: Optional[list[TSTypeParameter]] = None
    extends: Optional[str] = None
    implements: list[str] = field(default_factory=list)


@dataclass
class TSInterfaceDeclaration(TSNode):
    name: str
    members: list[TSPropertyDefinition | TSMethodDefinition]
    type_parameters: Optional[list[TSTypeParameter]] = None
    extends: list[str] = field(default_factory=list)


# ── Additional Control Flow ────────────────────────────────


@dataclass
class TSTernaryExpression(TSNode):
    condition: TSNode
    consequent: TSNode
    alternate: TSNode


@dataclass
class TSBreakStatement(TSNode):
    pass


@dataclass
class TSContinueStatement(TSNode):
    pass


@dataclass
class TSDoWhileStatement(TSNode):
    body: TSNode
    condition: TSNode


@dataclass
class TSSwitchStatement(TSNode):
    discriminant: TSNode
    cases: list[TSNode]  # TSCaseClause | TSDefaultClause


@dataclass
class TSCaseClause(TSNode):
    test: TSNode
    consequence: list[TSNode]


@dataclass
class TSDefaultClause(TSNode):
    consequence: list[TSNode]


@dataclass
class TSSequenceExpression(TSNode):
    expressions: list[TSNode]


# ── Try / Catch / Throw ──────────────────────────────────


@dataclass
class TSTryStatement(TSNode):
    body: TSBlockStatement
    handler: Optional[TSCatchClause]
    finalizer: Optional[TSBlockStatement]


@dataclass
class TSCatchClause(TSNode):
    param: TSIdentifier
    body: TSBlockStatement


@dataclass
class TSThrowStatement(TSNode):
    argument: TSNode
