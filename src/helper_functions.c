#include "helper_functions.h"


xmlNodePtr xmlNextSibling(xmlNodePtr cur) {
    return cur->next;
}

xmlNodePtr xmlPrevSibling(xmlNodePtr cur) {
    return cur->prev;
}
